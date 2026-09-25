#version 330

// PASADA DE RAYOS CREPUSCULARES (god rays): la luz que baja por el hueco
// del techo hasta la piscina.
//
// Es un efecto de pantalla, no de la escena: desde cada pixel se camina en
// linea recta hacia donde cae la luz cenital en pantalla, sumando lo que
// brilla por el camino con un peso que decae en cada paso. Donde hay algo
// brillante entre el pixel y la luz (el agua, las hadas, las molduras), el
// pixel recibe un rastro de esa luz: eso es lo que se lee como rayos que
// atraviesan el vapor.
//
// LO QUE BRILLA se lee del buffer del bloom, no del cuadro compuesto. El
// bloom ya es "solo lo que pasa el umbral, y en proporcion a cuanto lo
// pasa", que es exactamente lo que un rayo tiene que recoger. Leyendo el
// cuadro entero con un umbral propio, en una escena brillante casi todo
// pasaba el umbral y cada pixel acumulaba sesenta muestras de si mismo: la
// piscina entera se iba a blanco y la fuente quedaba envuelta en una
// neblina blanca en vez de cruzada por rayos.

in vec2 fragTexCoord;
out vec4 finalColor;

// El cuadro ya compuesto (bloom, niebla, ACES).
uniform sampler2D texture0;

// El halo del bloom, a mitad de resolucion: lo que brilla, ya umbralizado.
// Se lee con la Y invertida por lo mismo que en composite.fs: es un
// RenderTexture y sus filas van al reves de las de `texture0` tal como
// llega aca.
uniform sampler2D bloomTex;

// Donde cae la luz cenital en pantalla, en UV de 0 a 1. La proyecta el
// programa con la misma camara del trazador; fuera de pantalla se topa a
// los bordes.
uniform vec2 lightScreenPos;

// Cuanto se esparce el rastro hacia la luz (0.5: hasta la mitad del
// camino), cuanto pesa cada muestra, cuanto pierde cada paso, y la
// intensidad final.
uniform float density;
uniform float weight;
uniform float decay;
uniform float exposure;

// Cuantas muestras por pixel. Mas es mas suave y mas caro.
uniform int numSamples;

void main() {
    vec2 uv = fragTexCoord;
    vec2 deltaUV = (uv - lightScreenPos) * density / float(numSamples);

    vec3 color = texture(texture0, uv).rgb;
    vec3 godray = vec3(0.0);

    float illuminationDecay = 1.0;
    vec2 sampleUV = uv;

    for (int i = 0; i < numSamples; i++) {
        sampleUV -= deltaUV;
        vec2 s = clamp(sampleUV, 0.0, 1.0);
        vec3 samp = texture(bloomTex, vec2(s.x, 1.0 - s.y)).rgb;

        samp *= illuminationDecay * weight;
        godray += samp;
        illuminationDecay *= decay;
    }

    // Tenidos de cyan: es la luz de la fuente atravesando el vapor.
    godray *= vec3(0.7, 0.95, 1.0) * exposure;

    // SE MEZCLAN EN MODO PANTALLA, no se suman.
    //
    // Esta pasada corre DESPUES del mapeo de tono, o sea sobre un cuadro
    // que ya esta en 0..1 y ya no tiene ninguna curva que lo comprima.
    // Sumando, un rayo que cae sobre la piscina (que ahi ya vale 0.9) da
    // 1.4 y se recorta a blanco plano: se pierden el agua, las causticas
    // y todo lo que hubiera debajo. Medido en el climax de la cancion,
    // los god rays solos sumaban siete puntos de cuadro quemado.
    //
    // `1 - (1-a)(1-b)` es la mezcla en pantalla de toda la vida: se
    // comporta como una suma cuando los dos son oscuros y se acerca a uno
    // sin llegar nunca cuando se acumulan. Es, ademas, lo que hace
    // fisicamente una segunda exposicion sobre la misma pelicula.
    godray = clamp(godray, 0.0, 1.0);
    finalColor = vec4(1.0 - (1.0 - color) * (1.0 - godray), 1.0);
}
