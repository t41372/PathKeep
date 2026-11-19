// Lays one paint layer over the surface with Kubelka–Munk. The layer target
// holds the paint thickness (r) and the share of the second pigment (g).
// Optional wear takes paint off the high points of the weave inside worn
// patches, the way abrasion exposes the layers below on an old painting.

uniform sampler2D uSurface;
uniform sampler2D uLayer;
uniform vec3 uKa;
uniform vec3 uSa;
uniform vec3 uKb;
uniform vec3 uSb;
uniform float uThickness;
uniform float uImpasto;
uniform vec4 uWear;     // amount, patch size (ref px), height where wear starts, seed
uniform float uLosses;  // chance of small flakes lost down to the support
uniform float uLevel;   // how much a thick layer fills the weave's valleys

out vec4 outColor;

void main() {
  ivec2 ip = ivec2(gl_FragCoord.xy);
  vec4 s = texelFetch(uSurface, ip, 0);
  vec4 layer = texelFetch(uLayer, ip, 0);
  float x = layer.r * uThickness;

  if (uWear.x > 0.0) {
    vec2 rp = refPx();
    vec2 so = seedOffset(uWear.w);
    float patches = smoothstep(0.0, 0.3, fbm(rp / uWear.y + so, 5) + uWear.x - 0.55);
    float peak = smoothstep(uWear.z - 0.12, uWear.z + 0.1, s.a + 0.08 * gnoise(rp / 2.0 + so));
    float rub = patches * peak;
    float scuff = smoothstep(0.55, 0.85, fbm(vec2(rp.x / 140.0, rp.y / 9.0) + so + 4.0, 4) + uWear.x * 0.4);
    x *= 1.0 - clamp(rub * 0.92 + scuff * peak * 0.5, 0.0, 1.0);
  }
  if (uLosses > 0.0) {
    vec2 rp = refPx();
    vec2 so = seedOffset(uWear.w + 3.0);
    vec2 w = worley(rp / 34.0 + so + fbm2(rp / 20.0 + so, 3) * 0.35);
    float lost = smoothstep(0.2, 0.12, w.x) * step(1.0 - uLosses, hash12(rp / 34.0 + so + 0.5));
    x *= 1.0 - lost;
  }

  if (x <= 1e-5) {
    outColor = s;
    return;
  }
  vec3 K = mix(uKa, uKb, layer.g);
  vec3 S = mix(uSa, uSb, layer.g);
  // Paint settles into the valleys first, so a thick layer evens the surface out.
  float level = (1.0 - exp(-x * uLevel)) * 0.4;
  float height = mix(s.a, 0.5 + (s.a - 0.5) * 0.25, level) + x * uImpasto;
  outColor = vec4(kmOver(K, S, x, s.rgb), height);
}
