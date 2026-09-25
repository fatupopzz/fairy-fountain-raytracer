#version 330

// PASADA DE ACABADO: la ultima de la cadena, la que dibuja en la ventana.
//
// Cosas de lente, todas para que el cuadro no se lea como una imagen
// sintetica perfecta: lo que esta lejos del foco se desenfoca (profundidad
// de campo), las luces se estiran en estelas anamorficas, la lente se
// desarma en los bordes, los bordes se oscurecen (vinieta), encima de todo
// hay grano, y el cuadro se cierra al formato ancho.

in vec2 fragTexCoord;
out vec4 finalColor;

uniform sampler2D texture0;

// El cuadro CRUDO del trazador, del que solo interesa el alpha: la
// profundidad del impacto (0 pegado a la camara, 1 el fondo). Se lee con
// la Y invertida por lo mismo que en kaleidoscope.fs: `texture0` es un
// RenderTexture dibujado con alto negativo y esta es una textura normal.
uniform sampler2D depthTex;

// PROFUNDIDAD DE CAMPO. `focusDepth` es la profundidad (en la escala del
// alpha) que esta nitida: la distancia de la camara al centro de la
// fuente. `dofAmount` es el radio maximo del desenfoque, en UV: lo que
// esta lejos del foco (el borde de la cueva, el cielo) se desenfoca hasta
// ahi. Es lo que hace que la fuente se separe del fondo como en una foto
// con lente luminosa.
uniform float focusDepth;
uniform float dofAmount;

// ESTELAS ANAMORFICAS: cuanto se derraman en horizontal las luces. Es el
// artefacto de las lentes anamorficas de cine (las que comprimen la imagen
// para el formato ancho): su optica ovalada hace que un punto de luz se
// estire en una raya horizontal. Es, probablemente, lo que mas rapido hace
// que una imagen se lea como cine y no como render.
uniform float streak;

// El halo del bloom, a mitad de resolucion y ya desenfocado. De AHI salen
// las estelas y no del cuadro nitido: muestreando el cuadro, ocho puntos
// repartidos en un 10% de la pantalla caen cada diez pixeles y cada luz
// deja ocho COPIAS de si misma en fila en vez de una raya (se veia, y se
// medía). El halo, en cambio, ya viene suave, asi que las mismas ocho
// muestras se funden en una estela continua. Y de paso el umbral ya esta
// aplicado: el bloom es, por definicion, "solo lo que quema".
//
// Va con la Y sin invertir: `texture0` aca se dibuja con alto negativo,
// asi que `uv` ya viene dado vuelta respecto de composite.fs, que es
// donde este mismo halo si necesita el `1.0 - y`.
uniform sampler2D bloomTex;

// Las BANDAS NEGRAS de arriba y abajo, en fraccion de la altura. Crecen a
// lo largo del tema: la imagen se va cerrando al formato ancho.
uniform float letterbox;

// Cuanto se separan los canales, en coordenadas de textura. Sube en los
// golpes: es el latigazo que hace que el impacto se sienta en la lente y no
// solo en las luces.
uniform float chromatic;

// Cuanto grano se suma. Constante: el grano es la textura del soporte, no un
// efecto, y si respirara con la cancion dejaria de leerse como pelicula.
uniform float grainAmount;

// Para que el grano cambie de un cuadro al siguiente. Con esto quieto el
// ruido se congela y se ve como suciedad pegada a la pantalla.
uniform float time;

// Desenfoque minimo en toda la imagen, en UV.
//
// BAJO a la mitad (era 0.0010, casi un pixel a 800). El filtro difusor de
// base tiene sentido cuando la imagen de abajo es nitida; aca el cuadro ya
// viene de estirar 400x300 a 800x600, asi que era el TERCER ablandamiento
// encima del mismo pixel (bilineal, foco suave, y el disco del DOF) y lo
// unico que quedaba en pie era el realce CAS peleandolos a los tres.
const float SOFT_FOCUS = 0.0005;

// LA ZONA NITIDA, medida desde el plano de foco, en la escala del alpha
// (1.0 = `PROFUNDIDAD_MAXIMA`, o sea 50 unidades del mundo). Todo lo que
// caiga a menos de esto del foco esta perfectamente nitido; recien pasada
// la zona empieza el desenfoque.
//
// QUE EXISTA ESTA ZONA es el arreglo de un problema que se veia: parecia
// que la camara se re-enfocaba sola cada tanto en medio de la cancion.
//
// La causa NO era que el plano de foco se moviera. Se midio: la camara
// respira (el radio oscila 1.2 unidades cada 41 segundos) pero eso mueve
// a la camara y al sujeto JUNTOS, asi que la distancia relativa de cada
// objeto al foco no cambia. Lo que si cambia es por el PENDULO: la camara
// orbita 0.65 radianes a cada lado, y las cosas que estan fuera del eje
// —sobre todo los cristales de las esquinas de la plaza— se le acercan y
// se le alejan al girar. Con el desenfoque arrancando a 0.03 del foco (una
// unidad y media), los cristales cruzaban esa frontera: su borroneo
// variaba de 0.00 a 0.20 a lo largo del tema, y como son brillantes y
// refractivos, se notaba.
//
// CUANTO tiene que medir se calcula, no se prueba. La camara mira al
// centro de la fuente desde unas once unidades, y la fuente no es un
// punto: la esquina cercana de la plaza queda a nueve unidades del ojo y
// la lejana a dieciocho, o sea de 0.18 a 0.36 en la escala del alpha,
// contra un plano de foco que esta en 0.21. La mitad de ancho que hace
// falta para cubrirla entera es entonces 0.15, no 0.12.
//
// Y 0.12 no fallaba un poco: fallaba EXACTAMENTE en el borde. La esquina
// lejana de la plaza caia a 0.154 del foco, apenas pasado el limite, asi
// que vivia en el filo: cualquier movimiento chico de la camara la metia
// y la sacaba de la zona nitida, y como lo que hay ahi son los cristales
// —brillantes y refractivos— el cambio se veia. Leido desde la butaca eso
// no parece un objeto que se desenfoca: parece que la CAMARA SE
// REENFOCA sola cada tanto, que es justo lo que se reportaba.
//
// Con 0.20 (diez unidades del mundo) la zona nitida cubre la fuente
// entera con margen a los dos lados, y lo que se desenfoca sigue siendo
// lo que se quiere desenfocar: la pared del fondo de la cueva (0.46) y el
// cielo. El efecto de lente abierta no se pierde —lo que lo hace visible
// es el contraste entre la fuente nitida y el fondo blando, no que se le
// coma los bordes al sujeto.
//
// La forma de tres regiones con distancias explicitas (nitida, cerca y
// lejos), en vez de parametros de lente fisicos, es lo que recomienda GPU
// Gems 3 justamente para poder manejar esto a mano.
const float BANDA_NITIDA = 0.20;

// Cuanto tarda en llegar al desenfoque maximo despues de la zona nitida.
// La de ADELANTE es mas corta que la de atras a proposito: en una lente
// de verdad lo que esta mas cerca que el foco se desenfoca mucho mas
// rapido que lo que esta mas lejos, porque el circulo de confusion crece
// sin techo hacia el frente y se satura hacia el fondo.
const float RAMPA_CERCA = 0.10;
const float RAMPA_LEJOS = 0.26;

// El desenfoque de un punto a profundidad `d`: cero adentro de la zona
// nitida y creciendo despues, mas rapido hacia adelante que hacia atras.
float desenfoque(float d, float foco) {
    float fuera = abs(d - foco) - BANDA_NITIDA;
    if (fuera <= 0.0) {
        return 0.0;
    }
    float rampa = (d < foco) ? RAMPA_CERCA : RAMPA_LEJOS;
    return clamp(fuera / rampa, 0.0, 1.0);
}

// La vineta no oscurece a negro: oscurece a LAVANDA. Los bordes del
// cuadro se hunden en el mismo color que las sombras.
//
// Mas oscura que antes (era 0.55, 0.45, 0.80). Una vineta clara no cierra
// el cuadro: solo lo ensucia. Lo que hace que la mirada caiga en la fuente
// es que las esquinas esten VACIAS, y para eso tienen que bajar de verdad.
const vec3 VINETA_COLOR = vec3(0.30, 0.24, 0.50);

// Hasta donde llega la estela, en fraccion del ancho de pantalla.
const float ESTELA_LARGO = 0.11;

// El azul frio en que se derrama una lente anamorfica.
const vec3 ESTELA_TINTE = vec3(0.45, 0.65, 1.0);

float rand(vec2 co) {
    return fract(sin(dot(co, vec2(12.9898, 78.233))) * 43758.5453);
}

void main() {
    vec2 uv = fragTexCoord;

    // La separacion es RADIAL, no en una direccion fija: crece desde el
    // centro hacia afuera, que es como se desarma una lente de verdad. En el
    // centro los tres canales caen en el mismo punto y no se nota nada; en
    // las esquinas es donde se abre.
    vec2 dir = uv - vec2(0.5);

    // El rojo se corre hacia afuera y el azul hacia adentro; el verde queda
    // clavado. Moviendo dos de los tres, el desplazamiento aparente se
    // duplica sin tener que separar mas cada canal.
    float r = texture(texture0, uv + dir * chromatic).r;
    float g = texture(texture0, uv).g;
    float b = texture(texture0, uv - dir * chromatic).b;
    float a = texture(texture0, uv).a;

    vec3 color = vec3(r, g, b);

    // --- Profundidad de campo ---
    //
    // El circulo de confusion crece con la distancia al plano de foco,
    // con un umbral abajo para que lo que esta cerca del foco quede
    // perfectamente nitido. El desenfoque es un disco de doce muestras en
    // espiral (angulo aureo, para que no se vean ni cruces ni anillos),
    // promediadas con el centro. Cada muestra lee el cuadro ya compuesto,
    // asi que el bloom y los god rays se desenfocan con el.
    float depth = texture(depthTex, vec2(uv.x, 1.0 - uv.y)).a;
    // Mas un FOCO SUAVE de base: hasta lo que esta en foco lleva un
    // desenfoque minimo (un pixel y medio), como un filtro difusor delante
    // de la lente. Los bordes dejan de ser de raytracer y pasan a ser de
    // foto.
    float coc = SOFT_FOCUS + desenfoque(depth, focusDepth) * dofAmount;
    if (coc > 0.0002) {
        vec3 suma = color;
        float peso = 1.0;
        for (int i = 0; i < 12; i++) {
            float fi = float(i) + 1.0;
            float ang = fi * 2.39996;               // angulo aureo
            float rad = coc * sqrt(fi / 12.0);
            vec2 off = vec2(cos(ang), sin(ang)) * rad;
            // Las muestras que caen en algo MAS cercano y nitido no
            // deberian teñir el fondo (el primer plano no "sangra" hacia
            // atras); se les baja el peso segun cuan enfocadas estan.
            float d2 = texture(depthTex, vec2(uv.x + off.x, 1.0 - (uv.y + off.y))).a;
            float w = 0.3 + 0.7 * desenfoque(d2, focusDepth);
            suma += texture(texture0, uv + off).rgb * w;
            peso += w;
        }
        color = suma / peso;
    }

    // --- Estelas anamorficas ---
    //
    // Solo hacia los costados, y solo lo que QUEMA: se recoge lo que pasa
    // de `ESTELA_UMBRAL` a lo largo de una linea horizontal y se lo suma
    // teñido de azul. El peso cae con el cuadrado de la distancia, asi que
    // la estela se afina hacia las puntas en vez de ser una barra.
    if (streak > 0.0001) {
        // Se SUMAN las muestras, no se promedian. Promediadas, la estela
        // no puede pasar al halo del que sale y queda invisible debajo de
        // el; sumadas, un punto de luz reparte su energia a lo largo de la
        // linea y la raya aparece. `ESTELA_FUERZA` se encarga de que eso
        // no reviente el cuadro.
        // Cada muestra va AL CUADRADO antes de sumarse. Sin eso, la
        // estela mas fuerte del cuadro la produce la piscina, que es lo
        // que mas superficie brillante tiene, y el efecto se lee como una
        // mancha ancha y no como rayos. Elevar al cuadrado favorece a lo
        // intenso y chico (las hadas, el oro, las chispas del agua) sobre
        // lo tibio y grande, que es de donde salen las estelas en una
        // lente de verdad: de los puntos de luz.
        vec3 estela = vec3(0.0);
        for (int i = 1; i <= 10; i++) {
            float f = float(i) / 10.0;
            float dx = f * ESTELA_LARGO;
            float peso = (1.0 - f) * (1.0 - f);
            vec3 izq = texture(bloomTex, vec2(uv.x - dx, uv.y)).rgb;
            vec3 der = texture(bloomTex, vec2(uv.x + dx, uv.y)).rgb;
            estela += (izq * izq + der * der) * peso;
        }
        // En modo PANTALLA y no sumadas, por lo mismo que los god rays
        // (ver godrays.fs): esta pasada va despues del mapeo de tono, y
        // sumar ahi solo puede recortar. Medido en el climax, las estelas
        // sumadas se llevaban doce puntos del cuadro a blanco plano; en
        // modo pantalla se derraman sobre lo que hay sin borrarlo.
        vec3 rayas = clamp(estela * streak * ESTELA_TINTE, 0.0, 1.0);
        color = 1.0 - (1.0 - color) * (1.0 - rayas);
    }

    // Vinieta: los bordes se apagan apenas. El 0.7 controla cuanto: con
    // mas, las esquinas se comian la fuente, que tiene que ser brillante
    // hasta los bordes.
    vec2 vigUV = uv - 0.5;
    float vignette = 1.0 - dot(vigUV, vigUV) * 0.95;
    vignette = smoothstep(0.0, 1.0, clamp(vignette, 0.0, 1.0));
    color *= mix(VINETA_COLOR, vec3(1.0), vignette);

    // Ruido centrado en cero (el rand da 0..1, esto lo lleva a -1..1) para
    // que el grano no levante el brillo medio de la imagen: tiene que
    // ensuciar, no aclarar.
    float noise = rand(uv * time) * 2.0 - 1.0;
    color += noise * grainAmount;

    // Las bandas, al final de todo: son el borde del cuadro, no algo que
    // este pasando adentro de la escena, asi que ni el grano ni la vineta
    // las tocan. El borde va con `smoothstep` y no duro para que no se
    // vea el escalon del pixel.
    if (letterbox > 0.0001) {
        float borde = smoothstep(letterbox, letterbox + 0.004, uv.y)
            * smoothstep(letterbox, letterbox + 0.004, 1.0 - uv.y);
        color *= borde;
    }

    finalColor = vec4(clamp(color, 0.0, 1.0), a);
}
