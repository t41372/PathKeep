// One brush stroke, drawn as a ribbon and summed into the layer target.
//
// s runs along the stroke (0 where the brush lands, 1 where it lifts), t runs
// across it (-1..1). A flat brush lays paint in streaks (clumps of bristles,
// single hairs), starts loaded and runs dry along its length, and lifts off
// bristle by bristle. When it runs dry it only touches the high points of the
// surface underneath: that is the dry-brush skip.

uniform sampler2D uSurface;
uniform vec4 uRegion;      // x0, y0, x1, y1 in composition space; off when x1 < x0
uniform vec4 uRegionSoft;  // softness x, softness y, edge noise, seed

in vec2 vST;
in vec4 vA;   // seed, load, decay, dryness
in vec4 vB;   // bristle contrast, hair count, thickness, edge softness
in vec4 vC;   // taper (share of the length spent landing and lifting), puddle at the landing
in vec2 vDim; // length and width in reference px

out vec4 outColor;

float regionMask(vec2 nc) {
  if (uRegion.z < uRegion.x) return 1.0;
  vec2 so = seedOffset(uRegionSoft.w);
  // The field's outline wanders, then frays along the direction of the brushwork.
  vec2 q = nc + fbm2(nc * 2.4 + so + 29.0, 4) * uRegionSoft.z * 1.5;
  float ny = fbm(vec2(q.x * 2.6, q.y * 30.0) + so, 5) * uRegionSoft.z * 0.7;
  float nx = fbm(vec2(q.x * 9.0, q.y * 5.0) + so + 13.0, 5) * uRegionSoft.z * 0.7;
  float mx = smoothstep(uRegion.x - uRegionSoft.x, uRegion.x + uRegionSoft.x, q.x + nx)
    * (1.0 - smoothstep(uRegion.z - uRegionSoft.x, uRegion.z + uRegionSoft.x, q.x + nx));
  float my = smoothstep(uRegion.y - uRegionSoft.y, uRegion.y + uRegionSoft.y, q.y + ny)
    * (1.0 - smoothstep(uRegion.w - uRegionSoft.y, uRegion.w + uRegionSoft.y, q.y + ny));
  return mx * my;
}

void main() {
  float s = vST.x;
  float t = vST.y;
  float seed = vA.x;
  float load = vA.y;
  float decay = vA.z;
  float dryness = vA.w;
  float bristleContrast = vB.x;
  float hairs = vB.y;
  float thickness = vB.z;
  float soft = vB.w;
  float lenPx = vDim.x;
  vec2 so = seedOffset(seed);

  float across = t * 0.5 + 0.5;
  // Bristle lines drift a little sideways as the hand moves.
  float drift = gnoise(vec2(s * lenPx / 420.0, 0.5) + so) * 0.035;
  float a = across + drift;
  float clump = gnoise(vec2(a * hairs * 0.16, s * lenPx / 2600.0) + so + 3.1) * 0.5 + 0.5;
  float hair = gnoise(vec2(a * hairs, s * lenPx / 1100.0) + so + 7.7) * 0.5 + 0.5;
  float fine = gnoise(vec2(a * hairs * 2.7, s * lenPx / 600.0) + so + 1.9) * 0.5 + 0.5;
  float br = clump * 0.5 + hair * 0.33 + fine * 0.17;
  float bristle = mix(1.0, smoothstep(0.2, 0.8, br) * 1.3, bristleContrast);

  // Frayed sides.
  float side = gnoise(vec2(s * lenPx / 160.0, 2.0) + so) * 0.07 + gnoise(vec2(s * lenPx / 35.0, 5.0) + so) * 0.03;
  float edge = 1.0 - smoothstep(0.86 - soft * 0.45 + side, 1.0 + side, abs(t));

  // Each bristle lands and lifts at its own moment.
  float taper = vC.x;
  float land = taper * 0.5 * (gnoise(vec2(a * hairs * 0.35, 3.0) + so) * 0.5 + 0.5);
  float lift = 1.0 - taper * 0.8 * (gnoise(vec2(a * hairs * 0.45, 9.0) + so) * 0.5 + 0.5);
  float press = smoothstep(land, land + taper, s) * (1.0 - smoothstep(lift - taper * 1.4, lift, s));

  // Loaded at the start, starved toward the tail; maybe a puddle where it lands.
  float paint = load * exp(-decay * max(s, 0.0)) + vC.y * load * exp(-max(s, 0.0) * 30.0);
  paint *= bristle * press * edge;

  // Dry brush: low paint only reaches the high points underneath.
  float h = texelFetch(uSurface, ivec2(gl_FragCoord.xy), 0).a;
  float reach = h + (hair - 0.5) * 0.28 + (fine - 0.5) * 0.12;
  float threshold = dryness * (1.05 - clamp(paint, 0.0, 1.3) * 0.75);
  float deposit = dryness > 0.0 ? smoothstep(threshold - 0.05, threshold + 0.05, reach) : 1.0;

  float amount = paint * deposit * thickness * regionMask(normCoord());
  outColor = vec4(amount, 0.0, 0.0, 0.0);
}
