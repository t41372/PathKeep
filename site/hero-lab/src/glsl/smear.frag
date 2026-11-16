// Drags wet paint sideways, the way a squeegee or a wide, nearly dry brush
// pulled across the canvas carries colour into what is already there.
//
// The blade never presses evenly along its edge, so the drag comes in
// streaks; it starts and lifts at different points along its path. Only
// colour moves: the relief stays, so the canvas tooth and the grain still
// read sharply through the smear.

uniform sampler2D uSurface;
uniform float uSeed;
uniform vec4 uDrag;  // angle (radians), length (ref px), strength, coverage (0..1)
uniform vec4 uRows;  // streak width (ref px), streak contrast, start/stop scale (ref px), sideways wobble (ref px)
uniform vec2 uSpan;  // limit to a horizontal zone: centre y, half-height (composition space); off when half <= 0

out vec4 outColor;

void main() {
  ivec2 ip = ivec2(gl_FragCoord.xy);
  vec4 s = texelFetch(uSurface, ip, 0);
  vec2 rp = refPx();
  vec2 nc = normCoord();
  vec2 so = seedOffset(uSeed);

  vec2 dir = vec2(cos(uDrag.x), sin(uDrag.x));
  vec2 a = vec2(dot(rp, dir), dot(rp, vec2(-dir.y, dir.x)));
  float across = a.y + gnoise(vec2(a.x / 1600.0, a.y / 420.0) + so) * uRows.w;

  // Uneven pressure along the blade: streaks that run with the drag.
  float rows = gnoise(vec2(a.x / (uRows.x * 70.0), across / uRows.x) + so + 3.0) * 0.6
    + gnoise(vec2(a.x / (uRows.x * 30.0), across / (uRows.x * 0.35)) + so + 5.0) * 0.4;
  float press = smoothstep(-0.35, 0.55, rows * uRows.y + (uDrag.w - 0.5) * 2.0);
  // Where along its path the blade is touching at all.
  float touch = smoothstep(-0.25, 0.35, fbm(vec2(a.x / uRows.z, across / (uRows.z * 0.1)) + so + 7.0, 4) + uDrag.w - 0.5);
  float zone = 1.0;
  if (uSpan.y > 0.0) {
    float d = abs(nc.y - uSpan.x + 0.03 * fbm(vec2(nc.x * 3.0, 0.5) + so, 3));
    zone = 1.0 - smoothstep(uSpan.y * 0.55, uSpan.y, d);
  }
  float k = clamp(uDrag.z * press * touch * zone, 0.0, 1.0);
  if (k <= 0.002) {
    outColor = s;
    return;
  }

  // Colour carried from behind, most from close by. Each streak carries its own distance.
  float reach = 0.55 + 0.9 * (gnoise(vec2(across / (uRows.x * 3.0), 1.7) + so) * 0.5 + 0.5);
  float len = uDrag.y * reach / uRef;
  vec2 back = vec2(dir.x, -dir.y);
  vec3 acc = vec3(0.0);
  float wsum = 0.0;
  ivec2 hi = ivec2(uRes) - 1;
  const int N = 48;
  for (int i = 1; i <= N; i++) {
    float f = float(i) / float(N);
    ivec2 q = clamp(ivec2(gl_FragCoord.xy - back * f * len), ivec2(0), hi);
    float w = exp(-f * 2.2);
    acc += texelFetch(uSurface, q, 0).rgb * w;
    wsum += w;
  }
  outColor = vec4(mix(s.rgb, acc / wsum, k), s.a);
}
