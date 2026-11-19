// Shared by every pass: hashing, gradient noise, fBm, Worley cells, colour
// conversion and the two coordinate systems.
//
// `refPx()` is in reference pixels: a 3840-wide canvas, y down. Texture sizes
// (threads, paper tooth, bristles) are written in these units so a smaller
// render shows the same painting, only with less detail.
// `normCoord()` is the composition space: y down, height = 1, x up to aspect.

#define PI 3.14159265359

uniform vec2 uRes;
uniform float uRef;

vec2 refPx() { return vec2(gl_FragCoord.x, uRes.y - gl_FragCoord.y) * uRef; }
vec2 normCoord() { return vec2(gl_FragCoord.x, uRes.y - gl_FragCoord.y) / uRes.y; }

uint pkHash(uint x) {
  x ^= x >> 16u;
  x *= 0x7feb352du;
  x ^= x >> 15u;
  x *= 0x846ca68bu;
  x ^= x >> 16u;
  return x;
}

uint pkHash2(ivec2 p) {
  uvec2 q = uvec2(p + ivec2(1048576));
  return pkHash(q.x ^ pkHash(q.y + 0x9e3779b9u));
}

float hash12(vec2 p) { return float(pkHash2(ivec2(floor(p)))) / 4294967295.0; }

vec2 hash22(vec2 p) {
  uint h = pkHash2(ivec2(floor(p)));
  return vec2(float(h), float(pkHash(h ^ 0x68bc21ebu))) / 4294967295.0;
}

vec2 seedOffset(float seed) {
  return vec2(hash12(vec2(seed, 17.0)), hash12(vec2(seed, 91.0))) * 997.0;
}

vec2 gradDir(vec2 i) {
  float a = hash12(i) * 6.28318530718;
  return vec2(cos(a), sin(a));
}

// Gradient noise, roughly in [-1, 1].
float gnoise(vec2 p) {
  vec2 i = floor(p);
  vec2 f = p - i;
  vec2 u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
  float a = dot(gradDir(i), f);
  float b = dot(gradDir(i + vec2(1.0, 0.0)), f - vec2(1.0, 0.0));
  float c = dot(gradDir(i + vec2(0.0, 1.0)), f - vec2(0.0, 1.0));
  float d = dot(gradDir(i + vec2(1.0, 1.0)), f - vec2(1.0, 1.0));
  return mix(mix(a, b, u.x), mix(c, d, u.x), u.y) * 1.414;
}

// Fractal sum of rotated octaves, roughly in [-1, 1] (mostly within ±0.6).
float fbm(vec2 p, int octaves) {
  float sum = 0.0;
  float amp = 0.5;
  float norm = 0.0;
  for (int i = 0; i < 9; i++) {
    if (i >= octaves) break;
    sum += amp * gnoise(p);
    norm += amp;
    p = mat2(1.6, 1.2, -1.2, 1.6) * p + vec2(37.1, 17.3);
    amp *= 0.5;
  }
  return sum / norm;
}

vec2 fbm2(vec2 p, int octaves) {
  return vec2(fbm(p, octaves), fbm(p + vec2(41.3, 73.9), octaves));
}

// Distances to the nearest and second-nearest cell points.
vec2 worley(vec2 p) {
  vec2 i = floor(p);
  vec2 f = p - i;
  float f1 = 9.0;
  float f2 = 9.0;
  for (int y = -1; y <= 1; y++) {
    for (int x = -1; x <= 1; x++) {
      vec2 g = vec2(float(x), float(y));
      vec2 r = g + hash22(i + g) - f;
      float d = dot(r, r);
      if (d < f1) {
        f2 = f1;
        f1 = d;
      } else if (d < f2) {
        f2 = d;
      }
    }
  }
  return sqrt(vec2(f1, f2));
}

vec3 srgbToLinear(vec3 c) {
  return mix(c / 12.92, pow((c + 0.055) / 1.055, vec3(2.4)), step(0.04045, c));
}

vec3 linearToSrgb(vec3 c) {
  c = max(c, vec3(0.0));
  return mix(c * 12.92, 1.055 * pow(c, vec3(1.0 / 2.4)) - 0.055, step(0.0031308, c));
}

// Kubelka–Munk: one paint layer of thickness x (absorption K, scattering S
// per channel) laid over a surface whose reflectance is Rg.
vec3 kmOver(vec3 K, vec3 S, float x, vec3 Rg) {
  vec3 s = max(S, vec3(1e-4));
  vec3 a = 1.0 + K / s;
  vec3 b = sqrt(max(a * a - 1.0, vec3(1e-8)));
  vec3 bsx = min(b * s * x, vec3(15.0));
  vec3 sh = sinh(bsx);
  vec3 ch = cosh(bsx);
  vec3 c = a * sh + b * ch;
  vec3 R = sh / c;
  vec3 T = b / c;
  return R + T * T * Rg / (1.0 - R * Rg);
}
