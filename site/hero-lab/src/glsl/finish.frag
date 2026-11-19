// Turns the painted surface into the final image: light raking across the
// relief, an optional sheen, craquelure, a soft fall-off of light toward the
// edges, colour grading, grain, and a dithered conversion to 8-bit sRGB.

uniform sampler2D uSurface;
uniform float uSeed;
uniform float uRelief;   // how strongly height turns into light and shadow
uniform float uGloss;    // oil sheen; 0 for matte media
uniform float uCrack;    // craquelure darkness
uniform float uCrackSize;
uniform float uVignette;
uniform float uSaturation;
uniform vec3 uTint;      // multiplies the colour (linear)
uniform vec3 uLift;      // colour the darkest values are lifted toward (linear)
uniform float uLiftAmount;
uniform float uGrain;

out vec4 outColor;

float heightAt(ivec2 p) {
  ivec2 hi = ivec2(uRes) - 1;
  return texelFetch(uSurface, clamp(p, ivec2(0), hi), 0).a;
}

void main() {
  ivec2 ip = ivec2(gl_FragCoord.xy);
  vec4 s = texelFetch(uSurface, ip, 0);
  vec2 rp = refPx();
  vec2 so = seedOffset(uSeed);

  float dx = (heightAt(ip + ivec2(-1, 0)) - heightAt(ip + ivec2(1, 0))) / uRef;
  float dy = (heightAt(ip + ivec2(0, -1)) - heightAt(ip + ivec2(0, 1))) / uRef;
  vec3 n = normalize(vec3(dx * uRelief, dy * uRelief, 1.0));
  vec3 light = normalize(vec3(-0.52, 0.6, 0.6));
  float diffuse = dot(n, light) / light.z;
  vec3 c = s.rgb * mix(1.0, diffuse, 0.85);
  if (uGloss > 0.0) {
    vec3 half_ = normalize(light + vec3(0.0, 0.0, 1.0));
    float sheenMask = smoothstep(-0.3, 0.5, fbm(rp / 700.0 + so + 3.0, 4));
    c += pow(max(dot(n, half_), 0.0), 70.0) * uGloss * sheenMask;
  }

  if (uCrack > 0.0) {
    vec2 q = rp / uCrackSize;
    q += fbm2(q * 0.35 + so, 4) * 0.55;
    vec2 w = worley(q);
    float crack = 1.0 - smoothstep(0.0, 0.05, w.y - w.x);
    vec2 w2 = worley(q * 2.7 + 11.0);
    crack = max(crack, (1.0 - smoothstep(0.0, 0.04, w2.y - w2.x)) * 0.45);
    crack *= smoothstep(-0.25, 0.35, fbm(rp / 1100.0 + so + 9.0, 3));
    c *= 1.0 - crack * uCrack;
  }

  vec2 uv = gl_FragCoord.xy / uRes;
  vec2 d = (uv - 0.5) * vec2(uRes.x / uRes.y, 1.0);
  d += vec2(0.06, -0.04);
  float fall = smoothstep(0.35, 1.3, length(d * vec2(0.8, 1.05)));
  c *= 1.0 - uVignette * fall;

  float lum = dot(c, vec3(0.2126, 0.7152, 0.0722));
  c = mix(vec3(lum), c, uSaturation);
  c *= uTint;
  c = mix(c, uLift, uLiftAmount * (1.0 - smoothstep(0.0, 0.35, lum)));

  vec3 srgb = linearToSrgb(c);
  vec2 gp = gl_FragCoord.xy + so;
  float fineGrain = hash12(gp) + hash12(gp + 41.0) - 1.0;
  float clumpGrain = gnoise(rp / 2.2 + so + 21.0);
  srgb += (fineGrain * 0.65 + clumpGrain * 0.45) * uGrain * (0.6 + 0.4 * (1.0 - lum));
  srgb += (hash12(gp + 97.0) - hash12(gp + 13.0)) / 255.0;
  outColor = vec4(clamp(srgb, 0.0, 1.0), 1.0);
}
