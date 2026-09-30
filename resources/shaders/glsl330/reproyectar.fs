#version 330

// LA REPROYECCION (la idea del "timewarp" de los visores de realidad
// virtual, escrita aca desde cero).
//
// El trazador entrega un cuadro cada 25 a 40 ms, pero la pantalla se
// refresca cada 16. Mientras la CPU traza el cuadro siguiente, la GPU vuelve
// a dibujar el ULTIMO cuadro trazado movido a donde esta la camara AHORA:
// la camara es una funcion del segundo de la cancion, asi que en cada
// refresco se sabe exactamente donde esta. El giro de la camara alrededor
// de la isla (que es el movimiento que mas se nota) se ve a 60 cuadros por
// segundo aunque se trace a 30. Lo que se mueve por su cuenta (el hada, las
// chispas) sigue a la velocidad del trazado: esto solo corrige la camara.
//
// Es un mapeo HACIA ATRAS: para cada pixel de la pantalla se busca de donde
// viene en el cuadro viejo. Para eso haria falta la distancia de lo que se
// ve en ESTE pixel, que es justo lo que no se tiene (el cuadro nuevo no esta
// trazado). Se la estima con la del cuadro viejo y se itera: con la
// distancia leida en el punto de origen estimado se rehace el punto en el
// mundo, se lo proyecta con la camara vieja, y eso da un origen mejor. Con
// camaras que se movieron poco (un cuadro) tres vueltas alcanzan.
//
// La distancia viaja en el canal alfa del cuadro trazado, como fraccion de
// `profMax` (1 es el cielo: infinitamente lejos, y ahi solo cuenta el giro).

in vec2 fragTexCoord;
in vec4 fragColor;

uniform sampler2D texture0;

// Las dos camaras: posicion y los tres ejes (derecha, arriba, adelante).
uniform vec3 ojoAntes;
uniform vec3 derAntes;
uniform vec3 arrAntes;
uniform vec3 adeAntes;
uniform vec3 ojoAhora;
uniform vec3 derAhora;
uniform vec3 arrAhora;
uniform vec3 adeAhora;
// tan(campo visual / 2), el ancho sobre el alto, y la distancia maxima que
// codifica el alfa.
uniform float escala;
uniform float aspecto;
uniform float profMax;

out vec4 finalColor;

// Donde cae el punto `p` en la pantalla de la camara vieja, en UV de la
// imagen (v hacia abajo, como las filas del trazador).
vec2 proyectar_antes(vec3 p) {
    vec3 d = p - ojoAntes;
    float prof = max(dot(d, adeAntes), 1e-4);
    float x = dot(d, derAntes) / prof / (aspecto * escala);
    float y = dot(d, arrAntes) / prof / escala;
    return vec2((x + 1.0) * 0.5, (1.0 - y) * 0.5);
}

void main() {
    vec2 uv = fragTexCoord;
    // El rayo de ESTE pixel con la camara de ahora, como lo arma el trazador.
    float sx = (2.0 * uv.x - 1.0) * aspecto * escala;
    float sy = (1.0 - 2.0 * uv.y) * escala;
    vec3 rayo = normalize(derAhora * sx + arrAhora * sy + adeAhora);

    vec2 origen = uv;
    for (int k = 0; k < 3; k++) {
        float a = texture(texture0, clamp(origen, 0.0, 1.0)).a;
        // El cielo: tan lejos que solo importa hacia donde mira el rayo.
        float distancia = a >= 0.999 ? 1.0e4 : a * profMax;
        origen = proyectar_antes(ojoAhora + rayo * distancia);
    }
    origen = clamp(origen, vec2(0.0), vec2(1.0));
    // Color y distancia del origen: el post-procesado sigue leyendo la
    // distancia en el alfa (la niebla, el desenfoque).
    finalColor = texture(texture0, origen);
}
