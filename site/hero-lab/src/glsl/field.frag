// One colour field laid with a broad brush: even coverage inside a soft,
// fraying rectangle, with the brush's streaks running along its direction.
//
// The streaks are noise read in a frame turned to the brush angle and gently
// bent by a slow displacement, so they curve the way an arm sweeps, without
// the swirls a per-pixel rotation would make. Long, soft patches on top are
// the separate passes of the brush. With dryness the paint only catches the
// high points of the surface: a scumble.
// Writes thickness (r) and the share of the second pigment (g).

uniform sampler2D uSurface;
uniform float uSeed;
uniform vec4 uRegion;  // x0, y0, x1, y1 in composition space
uniform vec4 uEdge;    // softness across x, softness across y, outline wander, fray
uniform vec4 uPaint;   // base thickness, patchiness, streak contrast, dryness
uniform vec4 uBrush;   // angle (radians), streak width (ref px), streak length (ref px), cross-stroke share
uniform vec2 uRim;     // extra paint along the edge, rim width (composition units)
uniform vec4 uMix;     // base share of pigment B, variation, frequency, gradient down the field

out vec4 outColor;

float edgeBand(float v, float lo, float hi, float soft) {
  return smoothstep(lo - soft, lo + soft, v) * (1.0 - smoothstep(hi - soft, hi + soft, v));
}

vec2 alongAcross(vec2 p, float angle) {
  float c = cos(angle);
  float s = sin(angle);
  return vec2(c * p.x + s * p.y, -s * p.x + c * p.y);
}

float streaks(vec2 p, float angle, vec2 so) {
  vec2 a = alongAcross(p, angle);
  // Lines wander sideways along the sweep, so they are never quite parallel.
  float y = a.y + gnoise(vec2(a.x / (uBrush.z * 1.7), a.y / (uBrush.y * 30.0)) + so + 9.0) * uBrush.y * 1.3;
  float clumps = gnoise(vec2(a.x / (uBrush.z * 1.3), y / (uBrush.y * 1.7)) + so + 2.0);
  float hairs = gnoise(vec2(a.x / (uBrush.z * 0.7), y / (uBrush.y * 0.45)) + so + 3.0);
  float fine = gnoise(vec2(a.x / (uBrush.z * 0.35), y / (uBrush.y * 0.16)) + so + 4.0);
  // Streaks come and go along the sweep as the brush loads and runs dry.
  float presence = smoothstep(-0.45, 0.5, gnoise(vec2(a.x / (uBrush.z * 0.9), y / (uBrush.y * 6.0)) + so + 5.0));
  return (clumps * 0.5 + hairs * 0.32 + fine * 0.18) * (0.3 + 0.7 * presence);
}

void main() {
  ivec2 ip = ivec2(gl_FragCoord.xy);
  vec2 nc = normCoord();
  vec2 rp = refPx();
  vec2 so = seedOffset(uSeed);

  // Outline: wanders, then frays along the brush direction.
  vec2 q = nc + fbm2(nc * 2.2 + so + 29.0, 4) * uEdge.z;
  vec2 fa = alongAcross(rp, uBrush.x);
  float frayAcross = fbm(vec2(fa.x / 700.0, fa.y / 16.0) + so + 3.0, 4) * uEdge.w;
  float frayAlong = fbm(vec2(fa.x / 40.0, fa.y / 260.0) + so + 7.0, 4) * uEdge.w;
  float mx = edgeBand(q.x + frayAlong, uRegion.x, uRegion.z, uEdge.x);
  float my = edgeBand(q.y + frayAcross, uRegion.y, uRegion.w, uEdge.y);
  float mask = mx * my;
  if (mask <= 0.0) {
    outColor = vec4(0.0);
    return;
  }

  // The brush's sweep: a slow bend so streaks curve with the arm.
  vec2 p = rp + fbm2(rp / 1500.0 + so, 3) * 70.0;
  float s1 = streaks(p, uBrush.x, so);
  float s2 = streaks(p, uBrush.x + 0.55, so + 11.0);
  float s3 = streaks(p, uBrush.x - 0.5, so + 23.0);
  float crossMask = smoothstep(0.1, 0.6, fbm(nc * 3.5 + so + 31.0, 3));
  float streak = mix(s1, mix(s2, s3, step(0.0, fbm(nc * 2.0 + so + 37.0, 2))), uBrush.w * crossMask);

  // Separate passes of the brush: long soft patches along the sweep.
  vec2 a = alongAcross(p, uBrush.x);
  float passes = fbm(vec2(a.x / (uBrush.z * 2.2), a.y / (uBrush.y * 12.0)) + so + 1.0, 4);
  float drift = fbm(nc * 2.6 + so + 4.0, 4);

  float t = uPaint.x;
  t *= 1.0 + uPaint.y * (drift * 1.2 + passes * 0.8);
  t *= 1.0 + uPaint.z * streak;

  // A little more paint where the brush turned at the edge.
  float inner = min(min(q.x - uRegion.x, uRegion.z - q.x), min(q.y - uRegion.y, uRegion.w - q.y));
  t *= 1.0 + uRim.x * exp(-max(inner, 0.0) / max(uRim.y, 1e-4));

  if (uPaint.w > 0.0) {
    float hgt = texelFetch(uSurface, ip, 0).a;
    float load = clamp(t / max(uPaint.x, 1e-4), 0.0, 1.6);
    float th = uPaint.w * (1.15 - load * 0.45);
    t *= smoothstep(th - 0.07, th + 0.07, hgt + streak * 0.18);
  }

  float centreY = (uRegion.y + uRegion.w) * 0.5;
  float share = uMix.x + uMix.y * fbm(nc * uMix.z + so + 6.0, 4) + uMix.w * (nc.y - centreY);
  outColor = vec4(max(t, 0.0) * mask, clamp(share, 0.0, 1.0), 0.0, 0.0);
}
