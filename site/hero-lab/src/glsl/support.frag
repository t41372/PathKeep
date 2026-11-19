// The bare support the paint goes on: raw linen, cold-pressed paper or lime
// plaster. Writes reflectance (rgb, linear) and surface height (a, about 0..1).

uniform float uSeed;
uniform int uKind;      // 0 linen, 1 paper, 2 plaster
uniform vec3 uBase;     // linear colour of the bare material
uniform vec3 uFleck;    // linear colour of flecks (linen shives, paper inclusions)
uniform float uScale;   // texture size multiplier
uniform float uAge;     // 0..1 yellowing toward the edges and foxing
uniform float uRib;     // 0..1 horizontal threads heavier than vertical (a ribbed weave)

out vec4 outColor;

float linen(vec2 rp, vec2 so, out float shade, out float fleck) {
  float P = 7.5 * uScale;
  // Threads are never evenly spaced or straight: remap the coordinates.
  float u = rp.x / P + fbm(vec2(rp.x / (P * 9.0), 0.37) + so, 3) * 1.9
    + gnoise(vec2(rp.x / (P * 2.3), rp.y / (P * 55.0)) + so + 5.1) * 0.3;
  float v = rp.y / P + fbm(vec2(0.71, rp.y / (P * 9.0)) + so + 9.3, 3) * 1.9
    + gnoise(vec2(rp.x / (P * 55.0), rp.y / (P * 2.3)) + so + 2.7) * 0.3;
  float iu = floor(u);
  float fu = u - iu;
  float iv = floor(v);
  float fv = v - iv;

  // Linen is uneven: every thread has its own thickness and long slubs.
  float slubW = max(0.0, gnoise(vec2(iu * 3.1, rp.y / (P * 13.0)) + so + 1.3) - 0.2);
  float slubF = max(0.0, gnoise(vec2(rp.x / (P * 13.0), iv * 2.9) + so + 4.1) - 0.2);
  float tw = (0.74 + 0.3 * hash12(vec2(iu, 11.0) + so) + 0.45 * slubW) * (1.0 - 0.3 * uRib);
  float tf = (0.74 + 0.3 * hash12(vec2(13.0, iv) + so) + 0.45 * slubF) * (1.0 + 0.25 * uRib);
  float dw = abs(fu - 0.5) * 2.0 / tw;
  float df = abs(fv - 0.5) * 2.0 / tf;
  float pw = dw < 1.0 ? pow(cos(dw * PI * 0.5), 0.8) : 0.0;
  float pf = df < 1.0 ? pow(cos(df * PI * 0.5), 0.8) : 0.0;

  // Plain weave: each thread rises over one crossing and dips under the next.
  float sw = mod(iu, 2.0) < 1.0 ? 1.0 : -1.0;
  float sf = mod(iv, 2.0) < 1.0 ? 1.0 : -1.0;
  float ew = sw * cos(PI * (v - 0.5));
  float ef = -sf * cos(PI * (u - 0.5));
  // Some threads sit proud and some lie low; the over-under depth varies too.
  float liftW = (0.8 + 0.4 * hash12(vec2(iu, 29.0) + so)) * (1.0 - 0.25 * uRib);
  float liftF = (0.8 + 0.4 * hash12(vec2(31.0, iv) + so)) * (1.0 + 0.2 * uRib);
  float dipW = 0.1 + 0.12 * hash12(vec2(iu, 37.0) + so);
  float dipF = 0.1 + 0.12 * hash12(vec2(41.0, iv) + so);
  float hw = pw > 0.0 ? (0.42 + dipW * ew + 0.3 * pw + 0.14 * slubW) * liftW : 0.0;
  float hf = pf > 0.0 ? (0.42 + dipF * ef + 0.3 * pf + 0.14 * slubF) * liftF : 0.0;

  float fibW = gnoise(vec2(rp.x / 0.9, rp.y / 6.0) + so + 3.3);
  float fibF = gnoise(vec2(rp.x / 6.0, rp.y / 0.9) + so + 6.6);
  float h;
  if (hw >= hf) {
    h = hw + 0.05 * fibW;
    shade = 0.9 + 0.18 * hash12(vec2(iu, 5.0) + so) + 0.06 * gnoise(vec2(iu * 1.7, rp.y / 90.0) + so) + 0.03 * fibW;
  } else {
    h = hf + 0.05 * fibF;
    shade = 0.9 + 0.18 * hash12(vec2(7.0, iv) + so) + 0.06 * gnoise(vec2(rp.x / 90.0, iv * 1.7) + so) + 0.03 * fibF;
  }
  h = max(h, 0.16 + 0.03 * fibW);
  shade *= mix(0.86, 1.0, smoothstep(0.16, 0.45, h));

  // Shives: now and then a dark woody bit caught in the weave.
  float n1 = gnoise(vec2(rp.x / 5.0, rp.y / 13.0) + so + 11.0);
  float n2 = gnoise(vec2(rp.x / 61.0, rp.y / 53.0) + so + 23.0);
  fleck = smoothstep(0.72, 0.9, n1 * 0.55 + n2 * 0.75) * 0.45;
  return h;
}

float paper(vec2 rp, vec2 so, out float shade, out float fleck) {
  float s = uScale;
  // Cold-pressed tooth: rounded bumps of mixed sizes with winding valleys.
  vec2 warp = fbm2(rp / (38.0 * s) + so, 3) * 0.9;
  float tooth = fbm(rp / (9.0 * s) + warp + so, 3);
  float tooth2 = fbm(rp / (23.0 * s) + warp * 0.6 + so + 3.0, 3);
  float low = fbm(rp / (110.0 * s) + so + 7.0, 4) * 0.5 + 0.5;
  // Fibres: short streaks in every direction.
  float a = fbm(rp / (60.0 * s) + so + 2.0, 2) * PI * 2.0;
  vec2 dir = vec2(cos(a), sin(a));
  vec2 fq = vec2(dot(rp, dir), dot(rp, vec2(-dir.y, dir.x)));
  float fib = gnoise(vec2(fq.x / (8.0 * s), fq.y / (0.8 * s)) + so + 9.0);
  float h = 0.5 * smoothstep(-0.5, 0.55, tooth) + 0.26 * smoothstep(-0.55, 0.55, tooth2) + 0.14 * low + 0.06 + 0.04 * fib;
  shade = 0.985 + 0.025 * (low - 0.5) + 0.01 * fib;
  shade *= mix(0.955, 1.0, smoothstep(0.15, 0.6, h));
  float n = gnoise(rp / 3.0 + so + 31.0) * gnoise(rp / 47.0 + so + 5.0);
  fleck = smoothstep(0.45, 0.62, n) * 0.4;
  return h;
}

float plaster(vec2 rp, vec2 so, out float shade, out float fleck) {
  float s = uScale;
  float trowel = fbm(rp / (520.0 * s) + so, 4) * 0.5 + 0.5;
  float mid = fbm(rp / (46.0 * s) + so + 3.0, 5) * 0.5 + 0.5;
  float sand = gnoise(rp / (1.7 * s) + so + 8.0) * 0.5 + 0.5;
  vec2 w = worley(rp / (19.0 * s) + so * 0.7);
  float pitMask = step(0.8, hash12(rp / (19.0 * s) + so * 0.7 + 0.5));
  float pit = smoothstep(0.26, 0.04, w.x) * pitMask;
  float h = 0.18 + 0.34 * trowel + 0.34 * mid + 0.12 * sand - 0.32 * pit;
  shade = 0.96 + 0.06 * (mid - 0.5) + 0.03 * (sand - 0.5) - 0.12 * pit;
  float n = gnoise(rp / 4.0 + so + 13.0) * gnoise(rp / 61.0 + so + 3.0);
  fleck = smoothstep(0.4, 0.62, n) * 0.6;
  return h;
}

void main() {
  vec2 rp = refPx();
  vec2 so = seedOffset(uSeed);
  float shade = 1.0;
  float fleck = 0.0;
  float h;
  if (uKind == 0) {
    h = linen(rp, so, shade, fleck);
  } else if (uKind == 1) {
    h = paper(rp, so, shade, fleck);
  } else {
    h = plaster(rp, so, shade, fleck);
  }
  // Slow drifts in the material's own colour.
  float drift = fbm(rp / 900.0 + so + 50.0, 4);
  vec3 c = uBase * shade * (1.0 + 0.05 * drift);
  c = mix(c, uFleck, fleck);

  if (uAge > 0.0) {
    vec2 nc = normCoord();
    float aspect = uRes.x / uRes.y;
    vec2 d = abs(nc - vec2(aspect * 0.5, 0.5)) / vec2(aspect * 0.5, 0.5);
    float edge = smoothstep(0.55, 1.05, max(d.x, d.y) + 0.12 * fbm(nc * 3.0 + so, 3));
    c *= mix(vec3(1.0), vec3(0.98, 0.93, 0.82), edge * uAge);
    // Foxing: a few faint brown spots.
    vec2 wf = worley(rp / 260.0 + so + 77.0);
    float spot = smoothstep(0.16, 0.02, wf.x) * step(0.86, hash12(rp / 260.0 + so + 77.5));
    spot *= 0.6 + 0.4 * fbm(rp / 9.0 + so, 3);
    c *= mix(vec3(1.0), vec3(0.9, 0.8, 0.66), spot * uAge * 0.6);
  }
  outColor = vec4(c, h);
}
