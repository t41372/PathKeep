// One watercolour wash: a band of wet paint, how far its pigment spread, where
// it dried hard, where it bloomed, and where it settled into the paper.
//
// Each edge is a stack of fractal fronts. On wet paper they spread far apart
// (a soft bleed with feathered fingers); on dry paper they sit on top of each
// other and leave a darker tide line where the pigment was carried to the rim.
// Writes thickness (r) and the share of the second pigment (g).

uniform sampler2D uSurface;
uniform float uSeed;
uniform vec4 uBand;     // top, bottom, left, right
uniform float uTilt;    // slope of the top and bottom edges (dy per dx)
uniform vec4 uEdge;     // edge wander amplitude, wander frequency, softness top, softness bottom
uniform vec2 uSide;     // softness left, softness right
uniform float uWetVary; // how much the softness changes along an edge
uniform vec4 uDensity;  // base, mottling, pooling at the bottom edge, flat-wash streaks
uniform vec4 uTexture;  // granulation, flocculation, tide line, fringe
uniform vec4 uBloom;    // count, radius, strength, seed
uniform vec4 uMix;      // base share of pigment B, variation, frequency, gradient down the band
uniform int uShape;     // 0 band, 1 pool
uniform vec4 uPool;     // pool centre x, centre y, radius x, radius y
uniform vec2 uPoolShape; // pool rotation (radians), how lobed its outline is
uniform float uFringe;  // faint stain just outside a dried edge, as on raw canvas
uniform vec4 uHole;     // reserve left unpainted: centre x, centre y, radius x, radius y (off when rx <= 0)
uniform vec2 uHoleShape; // reserve edge softness, how lobed

out vec4 outColor;

float edgeCoverage(float d, float soft, vec2 nc, vec2 so, float h, out float rim) {
  float spread = mix(0.0016, 0.075, soft);
  float amp = mix(0.003, 0.05, soft);
  float reach = spread + amp * 1.4 + 0.004;
  rim = 0.0;
  if (d > reach) return 1.0;
  if (d < -reach) return 0.0;
  float freq = mix(46.0, 5.5, soft);
  float width = mix(0.0007, 0.02, soft);
  float acc = 0.0;
  const int K = 24;
  for (int k = 0; k < K; k++) {
    float fk = (float(k) + 0.5) / float(K);
    vec2 o = so + vec2(float(k) * 7.31, float(k) * 3.17);
    vec2 w = fbm2(nc * freq * 0.45 + o, 3) * 0.7;
    float n = fbm(nc * freq * (0.7 + 0.6 * fk) + w + o, 4);
    // Dried edges follow the paper's tooth.
    float front = d + n * amp + (fk - 0.5) * spread * 2.0 + (h - 0.5) * 0.0024 * (1.0 - soft);
    acc += smoothstep(-width, width, front);
    rim += exp(-front * front / (width * width * 9.0));
  }
  rim /= float(K);
  return acc / float(K);
}

float blooms(vec2 nc, vec2 so, vec4 box) {
  float f = 1.0;
  int count = int(uBloom.x);
  for (int i = 0; i < 12; i++) {
    if (i >= count) break;
    float fi = float(i);
    vec2 h = hash22(vec2(fi * 13.0 + uBloom.w, 7.0));
    float R = uBloom.y * (0.6 + 0.9 * hash12(vec2(fi * 5.0 + uBloom.w, 3.0)));
    // Backruns start where wetter paint meets a drying edge: sit astride an edge.
    float onTop = step(0.5, hash12(vec2(fi * 7.0 + uBloom.w, 11.0)));
    float edgeY = mix(box.w, box.y, onTop);
    float inward = mix(-1.0, 1.0, onTop) * R * (0.15 + 0.5 * h.y);
    vec2 c = vec2(mix(box.x, box.z, 0.08 + 0.84 * h.x), edgeY + inward);
    if (box.w - box.y > 0.8) c.y = mix(box.y, box.w, 0.15 + 0.7 * h.y);
    vec2 q = (nc - c) / R;
    if (dot(q, q) > 4.0) continue;
    vec2 bo = so + vec2(fi * 3.3, fi * 1.7);
    q *= vec2(1.0, 1.0 + 0.5 * h.x);
    q += fbm2(q * 1.4 + bo, 4) * 0.55;
    float r = length(q) + fbm(q * 7.0 + bo + 5.0, 5) * 0.14 + fbm(q * 23.0 + bo + 9.0, 4) * 0.045;
    float inside = 1.0 - smoothstep(0.965, 1.0, r);
    float rimB = exp(-pow((r - 1.0) / 0.02, 2.0));
    float pale = 0.42 + 0.34 * smoothstep(0.1, 1.0, r) + 0.16 * fbm(q * 4.0 + bo + 2.0, 3);
    f *= mix(1.0, pale, inside * uBloom.z);
    f *= 1.0 + rimB * uBloom.z * 1.1;
  }
  return f;
}

void main() {
  ivec2 ip = ivec2(gl_FragCoord.xy);
  vec2 nc = normCoord();
  vec2 rp = refPx();
  vec2 so = seedOffset(uSeed);
  float aspect = uRes.x / uRes.y;
  float h = texelFetch(uSurface, ip, 0).a;

  float cov;
  float tide;
  float pool;
  float halo = 0.0;
  vec4 box;
  float centreY;
  if (uShape == 1) {
    vec2 q = nc - uPool.xy;
    float cr = cos(uPoolShape.x);
    float sr = sin(uPoolShape.x);
    q = vec2(cr * q.x + sr * q.y, -sr * q.x + cr * q.y);
    vec2 qn = q / uPool.zw;
    vec2 lobes = fbm2(qn * 1.2 + so + 2.0, 4) * uPoolShape.y;
    float rn = length(qn + lobes);
    float dist = (1.0 - rn) * min(uPool.z, uPool.w);
    float soft = clamp(uEdge.z + fbm(nc * 2.4 + so + 5.0, 3) * uWetVary * 1.6, 0.0, 1.0);
    float rim;
    cov = edgeCoverage(dist, soft, nc, so + 1.0, h, rim);
    tide = rim * (1.0 - soft);
    pool = exp(-max(dist, 0.0) / 0.045) * smoothstep(-0.2, 0.7, qn.y);
    halo = uFringe * exp(-max(-dist, 0.0) / 0.007) * (1.0 - soft) * step(dist, 0.004);
    box = vec4(uPool.x - uPool.z, uPool.y - uPool.w, uPool.x + uPool.z, uPool.y + uPool.w);
    centreY = uPool.y;
  } else {
    float tilt = uTilt * (nc.x - aspect * 0.5);
    float wander = uEdge.x;
    float eTop = uBand.x + tilt + fbm(vec2(nc.x * uEdge.y, 1.3) + so, 4) * wander;
    float eBot = uBand.y + tilt + fbm(vec2(nc.x * uEdge.y, 7.9) + so, 4) * wander;
    float eLeft = uBand.z + fbm(vec2(2.1, nc.y * uEdge.y) + so, 4) * wander;
    float eRight = uBand.w + fbm(vec2(5.3, nc.y * uEdge.y) + so, 4) * wander;

    // Wet and dry stretches along the same edge.
    float vary = uWetVary;
    float softTop = clamp(uEdge.z + fbm(vec2(nc.x * 2.2, 3.3) + so, 3) * vary * 1.6, 0.0, 1.0);
    float softBot = clamp(uEdge.w + fbm(vec2(nc.x * 2.2, 8.8) + so, 3) * vary * 1.6, 0.0, 1.0);
    float softL = clamp(uSide.x + fbm(vec2(1.1, nc.y * 2.2) + so, 3) * vary * 1.6, 0.0, 1.0);
    float softR = clamp(uSide.y + fbm(vec2(4.4, nc.y * 2.2) + so, 3) * vary * 1.6, 0.0, 1.0);

    float rimT;
    float rimB = 0.0;
    float rimL = 0.0;
    float rimR = 0.0;
    cov = edgeCoverage(nc.y - eTop, softTop, nc, so + 1.0, h, rimT);
    if (cov > 0.0) cov *= edgeCoverage(eBot - nc.y, softBot, nc, so + 2.0, h, rimB);
    if (cov > 0.0) cov *= edgeCoverage(nc.x - eLeft, softL, nc, so + 3.0, h, rimL);
    if (cov > 0.0) cov *= edgeCoverage(eRight - nc.x, softR, nc, so + 4.0, h, rimR);
    tide = rimT * (1.0 - softTop) + rimB * (1.0 - softBot) + rimL * (1.0 - softL) + rimR * (1.0 - softR);
    pool = exp(-max(eBot - nc.y, 0.0) / 0.045);
    box = vec4(max(uBand.z, 0.0), max(uBand.x, 0.0), min(uBand.w, aspect), min(uBand.y, 1.0));
    centreY = (uBand.x + uBand.y) * 0.5;
  }
  if (uHole.z > 0.0) {
    vec2 hq = (nc - uHole.xy) / uHole.zw;
    hq += fbm2(hq * 1.3 + so + 17.0, 4) * uHoleShape.y;
    float hr = length(hq) + fbm(nc * 9.0 + so + 19.0, 4) * 0.06;
    cov *= smoothstep(1.0 - uHoleShape.x, 1.0 + uHoleShape.x * 0.3, hr);
  }
  if (cov <= 0.0 && halo <= 0.0) {
    outColor = vec4(0.0);
    return;
  }

  float dens = uDensity.x;
  dens *= 1.0 + uDensity.y * fbm(nc * 1.5 + so + 1.0, 5) * 1.4;
  dens *= 1.0 + uDensity.y * 0.6 * fbm(nc * 6.5 + so + 2.0, 4);
  dens *= 1.0 + uDensity.w * fbm(vec2(nc.x * 1.1, nc.y * 36.0) + so + 3.0, 4);
  dens *= 1.0 + uDensity.z * pool;
  dens *= 1.0 + uTexture.z * tide * 3.2;
  dens *= blooms(nc, so + 6.0, box);

  // Granulation: heavy pigment settles in the valleys of the paper, more in
  // some places than others.
  float settle = uTexture.x * (0.35 + 0.9 * smoothstep(-0.35, 0.45, fbm(nc * 2.6 + so + 8.0, 4)));
  dens *= max(0.0, 1.0 + settle * (0.5 - h) * 2.4);
  // Flocculation: pigment clumping a little above the tooth's scale.
  dens *= max(0.0, 1.0 + uTexture.y * fbm(rp / 11.0 + so + 4.0, 3) * 1.4);

  float share = uMix.x + uMix.y * fbm(nc * uMix.z + so + 6.0, 4) + uMix.w * (nc.y - centreY);
  float amount = max(dens, 0.0) * cov + halo * (1.0 - cov);
  outColor = vec4(amount, clamp(share, 0.0, 1.0), 0.0, 0.0);
}
