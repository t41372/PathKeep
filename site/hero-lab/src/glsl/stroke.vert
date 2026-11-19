// Ribbon vertices arrive in composition space (y down, height = 1).

uniform float uAspect;

in vec2 aPos;
in vec2 aST;
in vec4 aA;
in vec4 aB;
in vec4 aC;
in vec2 aDim;

out vec2 vST;
out vec4 vA;
out vec4 vB;
out vec4 vC;
out vec2 vDim;

void main() {
  gl_Position = vec4(aPos.x / uAspect * 2.0 - 1.0, 1.0 - aPos.y * 2.0, 0.0, 1.0);
  vST = aST;
  vA = aA;
  vB = aB;
  vC = aC;
  vDim = aDim;
}
