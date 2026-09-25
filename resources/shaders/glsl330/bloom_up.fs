#version 330

// SUBIDA DE LA CADENA DE MIPS DEL BLOOM: agranda un nivel al doble y lo
// mezcla sobre el de arriba. Se corre una vez por nivel, del mas chico al
// mas grande.
//
// El filtro es la carpa (tent) de tres por tres de la misma charla de
// Call of Duty: pesos 1-2-1 en cada eje, o sea un triangulo. Al agrandar,
// la interpolacion bilineal de la GPU ya deja la imagen suave, pero deja
// tambien los ROMBOS caracteristicos del bilineal, que sobre un halo se
// ven como facetas. La carpa los borra: es el nucleo justo para que la
// suma de la cadena entera quede sin estructura visible.
//
// SOBRE LA MEZCLA, QUE ES DONDE ESTO SE APARTA DEL PAPEL ORIGINAL. En Call
// of Duty (y en el articulo de LearnOpenGL que lo divulga) los niveles se
// SUMAN, con el framebuffer en punto flotante y mezcla aditiva. Aca los
// destinos son RenderTexture de raylib, que son de ocho bits por canal sin
// signo: sumar seis niveles ahi llega al techo de 1.0 en cualquier pixel
// con luz y el halo sale como una mancha blanca plana, que es justo lo que
// se venia de arreglar.
//
// Asi que en vez de sumar se INTERPOLA, que es lo que hace el bloom de
// Unity con su parametro "scatter": cada nivel entra como
// `mezcla(destino, carpa(origen), dispersion)`. El resultado no puede
// pasarse de 1 nunca, porque es un promedio ponderado de cosas que ya
// estan entre 0 y 1, y la forma del halo es la misma: con dispersion alta
// pesan mas los niveles chicos (halo ancho y difuso) y con dispersion baja
// los grandes (halo apretado pegado al objeto).
//
// La interpolacion se hace con la MEZCLA ALFA de la GPU y no dentro del
// shader: el shader escribe `dispersion` en el canal alfa y el mezclador
// de salida hace `destino*(1-a) + origen*a`. Hacerlo adentro seria leer y
// escribir la misma textura en la misma pasada, que no se puede.

in vec2 fragTexCoord;
out vec4 finalColor;

uniform sampler2D texture0;

// Radio del filtro, en coordenadas de textura del nivel que se LEE.
// Es la perilla del ancho del halo que mueve la cancion.
uniform float filterRadius;

// Cuanto pesa este nivel sobre el que ya estaba (0 a 1). Ver arriba.
uniform float scatter;

void main() {
    float x = filterRadius;
    float y = filterRadius;
    vec2 uv = fragTexCoord;

    vec3 a = texture(texture0, vec2(uv.x - x, uv.y + y)).rgb;
    vec3 b = texture(texture0, vec2(uv.x,     uv.y + y)).rgb;
    vec3 c = texture(texture0, vec2(uv.x + x, uv.y + y)).rgb;

    vec3 d = texture(texture0, vec2(uv.x - x, uv.y)).rgb;
    vec3 e = texture(texture0, vec2(uv.x,     uv.y)).rgb;
    vec3 f = texture(texture0, vec2(uv.x + x, uv.y)).rgb;

    vec3 g = texture(texture0, vec2(uv.x - x, uv.y - y)).rgb;
    vec3 h = texture(texture0, vec2(uv.x,     uv.y - y)).rgb;
    vec3 i = texture(texture0, vec2(uv.x + x, uv.y - y)).rgb;

    vec3 r = e * 4.0;
    r += (b + d + f + h) * 2.0;
    r += (a + c + g + i);
    r *= 1.0 / 16.0;

    finalColor = vec4(r, scatter);
}
