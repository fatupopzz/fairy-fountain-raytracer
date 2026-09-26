#version 330

// PASADA DE COMPOSICION: junta todo en un cuadro.
//
// Suma el bloom, mete la niebla usando la profundidad que el trazador dejo
// en el canal alpha, tine y comprime el rango con la curva ACES. La vinieta
// va al final de la cadena, en effects.fs.

in vec2 fragTexCoord;
in vec4 fragColor;

// La imagen cruda del trazador. RGB es el color; ALPHA es la profundidad
// codificada (0 = pegado a la camara, 1 = el fondo).
uniform sampler2D texture0;

// El halo ya desenfocado, a mitad de resolucion. Se samplea con las mismas
// coordenadas: la GPU lo estira con filtro bilineal y esa interpolacion
// suma suavidad en vez de restarla.
uniform sampler2D bloomTex;

uniform float bloomStrength;

uniform float fogDensity;
uniform vec3  fogColor;
uniform vec3  colorTint;
uniform vec2  resolution;

out vec4 finalColor;

// AgX: el transformador de imagen de Troy Sobotka, el que Blender adopto
// en la 4.0 en lugar de ACES.
//
// POR QUE CAMBIARLO. ACES tuerce el TONO de los colores saturados cuando
// se ponen brillantes: los rojos se van a naranja y los azules a violeta,
// y todo termina en blanco por el camino equivocado. En una escena que es
// casi toda rosa y cyan saturados eso es el problema central, y es lo que
// venia peleandose a mano con la mezcla de curvas: el rosa de las hadas
// dejaba de ser rosa justo cuando mas brillaba.
//
// AgX sube al blanco conservando el tono. La receta es: llevar el color a
// un espacio mas ancho (Rec. 2020), aplicar una matriz que lo "mete hacia
// adentro" (acerca los primarios, que es lo que evita que un canal se
// dispare solo), comprimir en LOGARITMO entre -12.5 y +4 pasos de
// diafragma con una sigmoide, y despues sacarlo de nuevo con la matriz
// inversa. Comprimir en logaritmo y no sobre el valor lineal es lo que
// hace que el rolloff sea parejo en todo el rango.
//
// Los numeros son los de la implementacion de three.js, que es la de
// Blender con la sigmoide reemplazada por un polinomio de grado seis que
// la aproxima (la original es una tabla de consulta).
// Pesos de luminancia (Rec. 709).
const vec3 LUMA = vec3(0.2126, 0.7152, 0.0722);

// El "look" que va encima de AgX: cuanto se abre el contraste y cuanto se
// empuja la saturacion. Ver el comentario dentro de `AgX`.
//
// La potencia subio de 1.25 a 1.45 cuando se midio que le pasaba al cuadro:
// el histograma entero vivia entre 0.18 y 0.72, o sea que NI UN PIXEL del
// cuadro era negro ni blanco. Una imagen sin los dos extremos se lee
// siempre como una foto velada, por buenos que sean los colores del medio.
const float LOOK_POTENCIA = 1.45;
const float LOOK_SATURACION = 1.30;

vec3 agxSigmoide(vec3 x) {
    vec3 x2 = x * x;
    vec3 x4 = x2 * x2;
    return 15.5 * x4 * x2
         - 40.14 * x4 * x
         + 31.96 * x4
         - 6.868 * x2 * x
         + 0.4298 * x2
         + 0.1191 * x
         - 0.00232;
}

vec3 AgX(vec3 color) {
    // Las matrices van por COLUMNAS, que es como las arma GLSL.
    const mat3 srgb_a_rec2020 = mat3(
        vec3(0.6274, 0.0691, 0.0164),
        vec3(0.3293, 0.9195, 0.0880),
        vec3(0.0433, 0.0113, 0.8956));
    const mat3 rec2020_a_srgb = mat3(
        vec3(1.6605, -0.1246, -0.0182),
        vec3(-0.5876, 1.1329, -0.1006),
        vec3(-0.0728, -0.0083, 1.1187));
    const mat3 entrada = mat3(
        vec3(0.856627153315983, 0.137318972929847, 0.11189821299995),
        vec3(0.0951212405381588, 0.761241990602591, 0.0767994186031903),
        vec3(0.0482516061458583, 0.101439036467562, 0.811302368396859));
    const mat3 salida = mat3(
        vec3(1.1271005818144368, -0.1413297634984383, -0.14132976349843826),
        vec3(-0.11060664309660323, 1.157823702216272, -0.11060664309660294),
        vec3(-0.016493938717834573, -0.016493938717834257, 1.2519364065950405));

    const float EV_MIN = -12.47393;
    const float EV_MAX = 4.026069;

    color = entrada * (srgb_a_rec2020 * color);

    // A escala logaritmica, normalizada al rango util. El maximo con 1e-10
    // es para que el logaritmo de cero no de menos infinito.
    color = log2(max(color, 1e-10));
    color = clamp((color - EV_MIN) / (EV_MAX - EV_MIN), 0.0, 1.0);
    color = agxSigmoide(color);

    // EL "LOOK".
    //
    // AgX entrega, a proposito, una imagen plana y poco saturada: su
    // trabajo es meter la escena adentro del rango de la pantalla sin
    // romper nada, no decidir como se ve. El contraste y el color se
    // ponen despues, y Blender trae presets para eso. Esto es el suyo
    // llamado "punchy", moderado: una potencia que abre el contraste y un
    // empujon de saturacion alrededor de la luminancia.
    //
    // Sin este paso la fuente se veia correcta y sin vida: medido, la
    // saturacion caia de 0.53 a 0.39 entre la version anterior y AgX
    // pelado.
    {
        float luma = dot(color, LUMA);
        color = pow(color, vec3(LOOK_POTENCIA));
        color = luma + LOOK_SATURACION * (color - luma);
        color = max(color, 0.0);
    }

    color = rec2020_a_srgb * (pow(max(salida * color, 0.0), vec3(2.2)));
    return clamp(color, 0.0, 1.0);
}

// Saturacion final. Despues de la curva y la niebla los colores quedan un
// poco tibios; 1.0 no toca nada, 1.15 empuja el rosa y el cyan sin
// reventar los blancos porque se aplica sobre el color ya comprimido.
const float SATURACION = 1.08;

// Exposicion: cuanto se calienta la entrada de la curva de tono. En lineal
// los medios del trazador quedan por 0.2, y la curva necesita entrada por
// encima de eso para no hundirlos.
const float EXPOSICION = 1.34;

// Levantamiento de negros, ya en sRGB: el negro mas negro del cuadro es
// este violeta y no el cero. Es lo que hace que las sombras se lean como
// aire oscuro de cueva y no como agujeros en la pantalla.
//
// A LA MITAD de lo que era (0.040, 0.030, 0.065). El levantamiento de
// negros es correcto como idea —las sombras de una cueva tienen color— y
// era un tercio del problema de que el cuadro se viera velado, porque se
// SUMA a la difusion del halo y a la niebla, que hacen lo mismo. Tres
// cosas levantando el negro a la vez no dan una sombra con color, dan
// gris. Con esto el piso del histograma baja de 0.18 a 0.06 y el violeta
// se sigue leyendo.
const vec3 LIFT = vec3(0.020, 0.015, 0.034);

// Cuanto realza el CAS, de 0 (apenas) a 1 (fuerte). A 0.65 le devuelve el
// filo al estirado sin que se note que hay un filtro puesto.
const float NITIDEZ = 0.80;

// El tinte del halo (ver "halacion" abajo).
const vec3 HALACION = vec3(1.0, 0.86, 0.96);

// Cuanto del halo entra como velo general, parejo por toda la imagen.
//
// BAJO de 0.22 a 0.07. Un filtro difusor delante de la lente es un efecto
// real y bonito, pero 0.22 sobre un bloom que la cancion lleva hasta 1.85
// significa que en el coro entra casi medio halo de velo plano: la cueva
// dejaba de tener sombras. Y lo peor es que un velo PAREJO no aporta
// profundidad, solo resta contraste; lo que hace bonito al difusor es que
// se concentre alrededor de lo que brilla, y de eso ya se encarga el halo
// tenido de `HALACION` que se suma arriba.
const float DIFUSION = 0.07;

// SPLIT TONING: las sombras se van a lavanda y las luces a crema rosada.
// Es la firma del look: nada es gris, lo oscuro es violeta y lo claro es
// calido. Se aplica en sRGB, sobre el color ya comprimido, pesado por la
// luminancia.
const vec3 TONO_SOMBRAS = vec3(0.86, 0.80, 1.10);
const vec3 TONO_LUCES = vec3(1.06, 0.98, 0.96);

void main() {
    // --- REALCE ADAPTATIVO POR CONTRASTE (CAS) ---
    //
    // El trazador entrega 400 x 300 y la pantalla son 800 x 600, asi que
    // la GPU estira el cuadro con filtro bilineal y eso, por definicion,
    // lo ablanda: cada pixel de pantalla es el promedio de dos texeles.
    // Esto le devuelve el filo, y es la mitad GPU de lo hibrido haciendo
    // lo que la mitad CPU no puede pagar (trazar el doble de pixeles
    // costaria cuatro veces mas).
    //
    // Es el CAS de AMD (FidelityFX, MIT), de Timothy Lottes, el mismo que
    // escribio FXAA. La gracia frente a un enfoque comun (restar un
    // desenfoque) es que la fuerza se ADAPTA por pixel: mide cuanto
    // contraste local hay y realza mas donde la imagen esta plana y menos
    // donde ya hay filo. Por eso no deja el halo blanco alrededor de los
    // bordes duros que delata a un "sharpen" de toda la vida, y aca eso
    // importa: en una escena de niebla y halos, un borde con reborde se
    // ve inmediatamente sintetico.
    //
    // Va ANTES del bloom y del mapeo de tono (opera sobre el cuadro
    // trazado, que es lo que perdio nitidez al estirarse) y antes de la
    // profundidad de campo, que vive en la ultima pasada: asi el fondo se
    // sigue pudiendo desenfocar despues sin que esto se lo devuelva.
    vec2 texel = 1.0 / resolution;
    vec3 e = texture(texture0, fragTexCoord).rgb;
    vec3 n = texture(texture0, fragTexCoord + vec2(0.0, -texel.y)).rgb;
    vec3 s = texture(texture0, fragTexCoord + vec2(0.0, texel.y)).rgb;
    vec3 o = texture(texture0, fragTexCoord + vec2(-texel.x, 0.0)).rgb;
    vec3 p = texture(texture0, fragTexCoord + vec2(texel.x, 0.0)).rgb;

    // El peso sale de la LUMINANCIA de la cruz, no de cada canal: con un
    // peso por canal, un borde entre dos colores de brillo parecido se
    // realza distinto en cada uno y aparece franja de color.
    float lE = dot(e, LUMA), lN = dot(n, LUMA), lS = dot(s, LUMA);
    float lO = dot(o, LUMA), lP = dot(p, LUMA);
    float minL = min(min(min(lO, lE), min(lP, lN)), lS);
    float maxL = max(max(max(lO, lE), max(lP, lN)), lS);

    // Cuanto margen queda hasta el negro y hasta el blanco: donde el pixel
    // ya esta contra un extremo, realzar solo puede recortar.
    float amp = clamp(min(minL, 1.0 - maxL) / max(maxL, 1e-4), 0.0, 1.0);
    amp = sqrt(amp);

    // `pico` va de -8 (realce suave) a -5 (fuerte) segun NITIDEZ.
    float pico = -(mix(8.0, 5.0, clamp(NITIDEZ, 0.0, 1.0)));
    float w = amp / pico;
    vec3 realzado = ((n + s + o + p) * w + e) / (1.0 + 4.0 * w);
    vec4 original = vec4(max(realzado, 0.0), texture(texture0, fragTexCoord).a);

    // --- A LINEAL ---
    //
    // El trazador entrega colores de 8 bits ya listos para pantalla (sRGB,
    // con la gamma puesta). Todo lo que sigue (sumar el halo, mezclar la
    // niebla, comprimir el rango) es aritmetica de LUZ y en sRGB sale mal:
    // sumar dos colores con gamma no da el doble de luz, y aplicar ACES y
    // gamma encima de datos que ya tenian gamma levantaba los negros a
    // gris y lavaba la saturacion. Asi que primero se deshace la gamma, se
    // opera en lineal, y la gamma se vuelve a poner al final, una sola vez.
    vec3 color = pow(original.rgb, vec3(2.2));

    // --- Bloom ---
    //
    // La Y va invertida al leer el halo. `bloomTex` es un RenderTexture, y
    // OpenGL numera las filas de un framebuffer de abajo hacia arriba
    // mientras que raylib dibuja de arriba hacia abajo: lo que se pinto en
    // la fila 0 quedo guardado en la ultima. Las pasadas de desenfoque
    // compensan eso con un alto negativo en el rectangulo de origen, pero
    // aca no se dibuja el halo, se lo SAMPLEA, y ahi no hay rectangulo donde
    // meter el signo. Sin este `1.0 - y` el resplandor sale de cabeza y
    // aparece abajo lo que brilla arriba.
    vec3 bloom = pow(texture(bloomTex, vec2(fragTexCoord.x, 1.0 - fragTexCoord.y)).rgb, vec3(2.2));
    // HALACION: el halo va tenido de rosa crema, como el que deja la
    // pelicula alrededor de una luz. Lo que brilla no se derrama en su
    // propio color exacto sino un poco mas calido, y eso es lo que hace
    // que la imagen se sienta sonada y no sintetica.
    color += bloom * HALACION * bloomStrength;
    // Y una DIFUSION general: un poco del halo entra parejo en toda la
    // imagen, como un filtro de niebla en la lente. Ablanda los negros
    // cerca de lo que brilla y le da a todo el cuadro un velo luminoso.
    color += bloom * DIFUSION;

    // --- Niebla ---
    //
    // CUADRATICA en la distancia (la ley que OpenGL llamaba GL_EXP2), no
    // lineal en el exponente: `1 - exp(-(d*k)^2)` en vez de
    // `1 - exp(-d*k)`.
    //
    // Las dos llegan al mismo velo total en el fondo; lo que cambia es el
    // reparto. La exponencial simple empieza a velar DESDE EL PRIMER
    // METRO: su derivada es maxima en cero. Con la densidad que pide la
    // cancion en el coro (0.18), la fuente —que esta a un tercio de la
    // profundidad maxima— se comia un 26% de niebla, y eso es lo que
    // hacia que el marmol de las columnas se viera lavado y que la
    // piscina, que tiene que ser lo mas saturado del cuadro, saliera
    // celeste palido.
    //
    // La cuadratica arranca con derivada CERO: el primer tercio queda
    // practicamente limpio (9% en vez de 26%), y despues la niebla sube
    // rapido. El 5.3 esta elegido para que en el FONDO las dos leyes den
    // exactamente lo mismo (59% de velo al fondo con esa densidad), asi
    // que el borde de la cueva y el cielo se ven igual que antes y lo
    // unico que cambio es que la fuente salio de la niebla. Eso es
    // ademas lo que hace de verdad la atmosfera —el aire cercano no vela
    // nada y la distancia si— y lo que le da al cuadro la separacion
    // entre planos que no tenia.
    float depth = original.a;
    float d = fogDensity * depth * 5.3;
    float fogFactor = 1.0 - exp(-d * d);
    color = mix(color, pow(fogColor, vec3(2.2)), fogFactor);

    // --- Tinte global ---
    // Empuja la paleta entera hacia el teal de la fuente, y un poco de rosa
    // en el coro.
    color *= colorTint;

    // --- Compresion de rango (AgX) ---
    // El bloom suma sin techo, asi que un foco puede pasarse de 1 por
    // mucho. Recortar ahi dejaria discos blancos planos; AgX lo sube al
    // blanco sin torcerle el tono por el camino.
    color = AgX(color * EXPOSICION);

    // --- Gamma ---
    // La unica vuelta a sRGB de toda la cadena.
    color = pow(color, vec3(1.0 / 2.2));

    // --- Levantamiento de negros ---
    // Se suma en proporcion a lo oscuro que es el pixel: las luces no se
    // enteran, las sombras suben al violeta de LIFT.
    color += LIFT * (1.0 - color);

    // --- Split toning ---
    float luma = dot(color, vec3(0.2126, 0.7152, 0.0722));
    color *= mix(TONO_SOMBRAS, TONO_LUCES, smoothstep(0.1, 0.8, luma));

    // --- Saturacion ---
    luma = dot(color, vec3(0.2126, 0.7152, 0.0722));
    color = clamp(mix(vec3(luma), color, SATURACION), 0.0, 1.0);

    finalColor = vec4(color, 1.0);
}
