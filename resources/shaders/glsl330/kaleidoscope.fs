#version 330

// PASADA DE CALEIDOSCOPIO: pliega EL FONDO, no el cuadro entero.
//
// Toma el cuadro ya compuesto (bloom, niebla, tinte, vinieta) y lo pliega
// sobre si mismo en N sectores iguales. La imagen no se repite girada: se
// ESPEJA en cada sector, que es lo que hace el juguete de verdad y lo que
// hace que las simetrias cierren en vez de tener un corte visible en cada
// division.
//
// EL PLIEGUE NO TOCA EL PRIMER PLANO. Plegando todo, el escenario
// desaparecia: la esfera, los postes y el piso se convertian en pedazos de
// mandala y no quedaba nada que leer como lugar. Con la profundidad como
// mascara, el efecto se queda donde no hay nada que arruinar (la pared del
// fondo y el techo) y el escenario se sostiene entero adelante. El fondo
// termina siendo una mandala hecha con las luces del propio rig.

in vec2 fragTexCoord;
out vec4 finalColor;

uniform sampler2D texture0;

// El cuadro CRUDO del trazador, del que aca solo interesa el alpha: la
// profundidad del impacto (0 pegado a la camara, 1 el fondo).
uniform sampler2D depthTex;

// Entre que profundidades se abre el efecto: `x` donde todavia vale cero,
// `y` donde ya vale uno. Entre las dos hay un desvanecido suave; con un
// corte duro se veria el contorno de los objetos recortado contra el fondo
// plegado, como un collage.
uniform vec2 depthRange;

// En cuantos sectores se parte el circulo. 4 son cuatro cuadrantes muy
// legibles; 12 ya es una mandala fina.
uniform float segments;

// Cuanto gira el patron, en radianes. Girando esto con el tiempo el
// caleidoscopio no se queda quieto, y como el pliegue es simetrico el giro
// no tiene principio ni final visible.
uniform float rotation;

// Cuanto del efecto se aplica: 0 deja el cuadro intacto, 1 es caleidoscopio
// puro. En el medio es una doble exposicion del cuadro consigo mismo.
uniform float kMix;

// El eje del pliegue, normalmente el centro de la pantalla.
uniform vec2 center;

void main() {
    vec2 uv = fragTexCoord;

    // A polares alrededor del centro: el pliegue es una operacion sobre el
    // ANGULO y no sobre la posicion, asi que en cartesianas no se puede
    // escribir.
    vec2 delta = uv - center;
    float r = length(delta);
    float angle = atan(delta.y, delta.x) + rotation;

    // El pliegue. `mod` mete el angulo dentro de un sector, y el `if` da
    // vuelta la segunda mitad del sector sobre la primera: ahi esta el
    // espejo. Sin ese reflejo cada division mostraria una copia rotada y se
    // veria la juntura como un tajo.
    float segAngle = 2.0 * 3.14159265 / segments;
    float a = mod(angle, segAngle);
    if (a > segAngle * 0.5) {
        a = segAngle - a;
    }

    // De vuelta a cartesianas, con el mismo radio: el pliegue no acerca ni
    // aleja nada del centro, solo decide de que direccion se lee.
    vec2 kalUv = center + r * vec2(cos(a - rotation), sin(a - rotation));
    // Las esquinas quedan a mas de 0.5 del centro, asi que hay direcciones
    // en las que el radio se sale de la textura. El clamp las pega al borde
    // en vez de dejar que el modo de repeticion traiga el lado opuesto.
    kalUv = clamp(kalUv, 0.0, 1.0);

    vec4 kalColor = texture(texture0, kalUv);
    vec4 origColor = texture(texture0, uv);

    // La mascara de profundidad.
    //
    // La Y va invertida al leer `depthTex`. `texture0` es un RenderTexture y
    // se dibuja con el alto negativo (OpenGL numera las filas al reves), asi
    // que `fragTexCoord` viene ya dado vuelta; `depthTex` en cambio es una
    // textura normal subida desde memoria, con las filas en el orden de
    // siempre. Sin este `1.0 - y` la mascara saldria espejada y el efecto se
    // comeria el piso en vez del techo.
    float depth = texture(depthTex, vec2(uv.x, 1.0 - uv.y)).a;
    float fondo = smoothstep(depthRange.x, depthRange.y, depth);

    finalColor = mix(origColor, kalColor, kMix * fondo);
}
