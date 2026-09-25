#version 330

// BAJADA DE LA CADENA DE MIPS DEL BLOOM: reduce la imagen a la mitad, una
// vez por nivel.
//
// Es el filtro de trece muestras de la charla de Jorge Jimenez en el
// SIGGRAPH 2014 sobre el post-procesado de Call of Duty: Advanced
// Warfare, que es lo que usan hoy casi todos los motores.
//
// POR QUE ESTO Y NO UN GAUSSIANO. El bloom anterior era lo de siempre: un
// gaussiano separable de trece muestras, repetido dos veces, todo a la
// misma resolucion, con la separacion entre muestras como perilla de
// radio. Funciona, pero tiene un techo: el radio del halo no puede pasar
// de las muestras que uno paga. Para derramar la luz por medio cuadro hay
// que separar las muestras hasta que se vean SUELTAS, y ahi el gaussiano
// deja de ser un desenfoque y pasa a ser trece copias en fila. Se veia en
// esta escena: el brillo detras de la Triforce no era un resplandor, era
// una bola de algodon blanca de borde reconocible.
//
// La cadena de mips resuelve eso cambiando de estrategia. En vez de
// agrandar el nucleo, se ACHICA la imagen: seis veces a la mitad, y en el
// nivel mas chico (doce por nueve pixeles) un filtro de tres pixeles cubre
// un cuarto de la pantalla. Despues se vuelve a subir sumando cada nivel
// sobre el anterior. El halo que sale es la suma de seis desenfoques de
// escalas distintas, que es como se ve la luz dispersandose de verdad: un
// nucleo apretado y brillante con una falda enorme y tenue que llega hasta
// el borde del cuadro. Y sale BARATO, porque los cinco niveles chicos
// juntos son un tercio de un solo nivel grande.
//
// El filtro de trece muestras, en vez de las cuatro que bastarian para
// reducir a la mitad, es para que no aparezca ALIASING entre niveles: al
// reducir tan seguido, una luz chica y brillante que caiga entre dos
// texeles aparece y desaparece cuando la camara se mueve, y eso se ve como
// un parpadeo que en una escena de puntos de luz —que es exactamente esta
// escena— es insoportable. Las trece muestras cubren un area de cuatro por
// cuatro texeles del origen y le quitan al resultado las frecuencias que el
// nivel siguiente no puede representar.

in vec2 fragTexCoord;
out vec4 finalColor;

uniform sampler2D texture0;

// Tamano en pixeles del nivel que se esta LEYENDO (el grande).
uniform vec2 srcResolution;

void main() {
    vec2 t = 1.0 / srcResolution;
    float x = t.x;
    float y = t.y;
    vec2 uv = fragTexCoord;

    // Las trece muestras: un anillo exterior de nueve separadas dos
    // texeles (a..i) y un cuadrado interior de cuatro separadas uno
    // (j..m), que es el que lleva la mitad del peso.
    vec3 a = texture(texture0, vec2(uv.x - 2*x, uv.y + 2*y)).rgb;
    vec3 b = texture(texture0, vec2(uv.x,       uv.y + 2*y)).rgb;
    vec3 c = texture(texture0, vec2(uv.x + 2*x, uv.y + 2*y)).rgb;

    vec3 d = texture(texture0, vec2(uv.x - 2*x, uv.y)).rgb;
    vec3 e = texture(texture0, vec2(uv.x,       uv.y)).rgb;
    vec3 f = texture(texture0, vec2(uv.x + 2*x, uv.y)).rgb;

    vec3 g = texture(texture0, vec2(uv.x - 2*x, uv.y - 2*y)).rgb;
    vec3 h = texture(texture0, vec2(uv.x,       uv.y - 2*y)).rgb;
    vec3 i = texture(texture0, vec2(uv.x + 2*x, uv.y - 2*y)).rgb;

    vec3 j = texture(texture0, vec2(uv.x - x, uv.y + y)).rgb;
    vec3 k = texture(texture0, vec2(uv.x + x, uv.y + y)).rgb;
    vec3 l = texture(texture0, vec2(uv.x - x, uv.y - y)).rgb;
    vec3 m = texture(texture0, vec2(uv.x + x, uv.y - y)).rgb;

    // Los pesos suman exactamente 1: 0.125 + 4*0.03125 + 4*0.0625 +
    // 4*0.125. Que sumen uno es lo que hace que bajar la cadena no cambie
    // el brillo medio del halo, y por lo tanto que la intensidad del bloom
    // no dependa de cuantos niveles tenga.
    vec3 r = e * 0.125;
    r += (a + c + g + i) * 0.03125;
    r += (b + d + f + h) * 0.0625;
    r += (j + k + l + m) * 0.125;

    finalColor = vec4(r, 1.0);
}
