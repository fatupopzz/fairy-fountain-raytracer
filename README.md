# Great Fairy Fountain — un diorama trazado con rayos

Un trazador de rayos en tiempo real, escrito desde cero en Rust, que dibuja la
fuente del Hada Mayor de *Ocarina of Time* sobre una **isla flotante** y la
anima con el **Great Fairy's Fountain Theme**. En el escalón de la entrada,
**Link** —hecho de cubos— toca la Ocarina del Tiempo mientras **Navi** le da
vueltas a la cabeza; de la ocarina salen notas con cada nota del arpa, el agua
late con cada tiempo, y cuando entran las voces una columna de luz sube de la
Trifuerza al cielo bajo una **aurora boreal**.

No hay motor 3D: la geometría, la iluminación, las sombras, los reflejos, el
modelo de Link y el cielo son código propio. Lo único que aporta raylib es la
ventana, el audio y la capacidad de correr shaders de fragmentos.

Las únicas dependencias son `raylib` (ventana, entrada, audio, shaders),
`rayon` (repartir las filas entre hilos) e `image` (leer y escribir PNG).
Ni matemática de vectores, ni intersecciones, ni BVH, ni sombreado.

## Video

**[▶ Ver el video completo (docs/fuente_de_las_hadas.mp4)](docs/fuente_de_las_hadas.mp4)**
— la canción entera, 1280×960, grabada con `--video` (48 MB; la versión en
alta calidad, de casi 100 MB, se regenera con el mismo comando y no va al repo).

[![La fuente de noche](docs/portada.jpg)](docs/fuente_de_las_hadas.mp4)

---

## Correrlo

Hace falta [Rust](https://rustup.rs). El mp3 y el análisis ya vienen en el repo.

```bash
cargo run --release
```

Tiene que correrse desde la raíz del proyecto: las rutas de las texturas, los
shaders y la música son relativas a ahí.

### Controles

| | |
|---|---|
| `espacio` | play / pausa |
| `←` `→` o `A` `D` | girar alrededor de la isla |
| `↑` `↓` o `W` `S` | subir y bajar la cámara |
| `Q` `E` o `PgUp` `PgDn` | acercar y alejar |
| ratón | arrastrar para orbitar, rueda para acercar |
| `X` | antialiasing 2×2 (cuadruplica el costo) |
| `T` | antialiasing temporal (gratis, viene encendido) |
| `F` | guardar una foto del cuadro en pantalla |
| `H` | mostrar u ocultar la ayuda |

La cámara da vueltas sola alrededor de la isla siguiendo un programa de planos
que marca la canción (ver más abajo); las teclas y el ratón se **suman** a ese
recorrido en vez de reemplazarlo.

### Otros modos

```bash
cargo run --release -- --video docs/fuente_de_las_hadas.mp4   # graba el video (pide ffmpeg)
cargo run --release -- --foto 95      # traza ese segundo con todo el post-procesado y lo guarda
cargo run --release -- --bench        # mide cinco momentos y vuelca el trazado crudo
cargo run --release -- --sync 20      # imprime, segundo a segundo, lo que el análisis le pide a la escena
cargo run --release -- --taa          # banco de pruebas del antialiasing temporal
cargo run --release -- --sin-taa      # apagarlo, para compararlo
```

El video no corre en tiempo real: cada cuadro se traza al doble de resolución
(800×600) con cuatro muestras por píxel y pasa por el mismo post-procesado que
en vivo; el tiempo lo pone el número de cuadro, así que sale a 30 fps exactos.
`VIDEO_DESDE` y `VIDEO_HASTA` recortan un tramo, `BENCH_T=46,96,136` elige qué
segundos mide `--bench`, y `CAMARA=x,y,z,mx,my,mz` fija la cámara en `--foto`.

---

## Lo que pide la consigna

| | |
|---|---|
| **Escena compleja** | Isla flotante de bloques con cascadas, cristales colgantes e islotes; la fuente con piscina, seis columnas, techo, altar y Trifuerza; tres anillos toroidales en precesión; Link articulado de ~50 cubos; Navi; notas; ~80 hadas y motas; rupias; antorchas; skybox con aurora y mar de nubes. 126 objetos, 10 luces de color. |
| **Rotación y zoom** | La cámara orbita 360° alrededor de la isla (una vuelta cada 96 s) y se acerca y aleja con el programa de planos; teclado y ratón suman giro, altura y distancia. |
| **Materiales** (cada uno con su textura y sus pesos de albedo, especular, reflexión y transparencia) | Ver la tabla de abajo: son más de diez. |
| **Refracción** | El agua de la piscina y de las cascadas (1.33), los cristales (1.5), las rupias (1.6). Con Fresnel: el agua es espejo mirada de costado y transparente mirada de frente. |
| **Reflexión** | El agua, el mármol pulido de la plaza, la obsidiana del altar, el oro, la ocarina esmaltada, el escudo. Con reflejos rugosos donde el material no es espejo. |
| **Skybox** | Equirectangular generado por código al arrancar: estrellas que titilan, nebulosa, luna, y encima, calculado por rayo, la aurora, el mar de nubes, las estrellas fugaces y el amanecer. |

### Los materiales

Los cuatro pesos del albedo son `[difuso, especular, reflexión, transparencia]`.

| Material | Textura | Albedo | Especular | Índice de refracción |
|---|---|---|---|---|
| Mármol de hada (bordes, columnas, techo) | `fairy_marble.png` + relieve | `[1.0, 0.22, 0.10, 0.0]` | 18 | — |
| Mármol pulido (plaza) | `fairy_marble.png` + relieve | `[0.9, 0.6, 0.30, 0.0]` | 80 | — |
| Agua | `water_fairy.png`, desplazándose | `[0.25, 0.3, 0.5, 0.6]` | 120 | 1.33 |
| Oro (molduras) | `gold_triforce.png` + relieve | `[1.0, 0.95, 0.35, 0.0]` | 95 | — |
| Obsidiana (altar) | `obsidian.png` + relieve | `[1.0, 0.7, 0.6, 0.0]` | 200 | — |
| Cristal | `crystal.png` | `[0.35, 0.9, 0.25, 0.7]` | 120 | 1.5 |
| Piedra de la isla | `stone_cave.png` + relieve | `[1.0, 0.05, 0.0, 0.0]` | 6 | — |
| Pasto | pintada por código | `[1.0, 0.06, 0.0, 0.0]` | 10 | — |
| Tierra | pintada por código | `[1.0, 0.04, 0.0, 0.0]` | 8 | — |
| Cascada | `water_fairy.png`, cayendo | `[0.35, 0.5, 0.25, 0.45]` | 90 | 1.33 |
| Rupia | color | `[0.3, 1.0, 0.3, 0.55]` | 180 | 1.6 |
| Túnica de Link | tela pintada por código | `[1.0, 0.08, 0.0, 0.0]` | 12 | — |
| Escudo hyliano | pintado por código | `[1.0, 0.7, 0.25, 0.0]` | 60 | — |
| Ocarina del Tiempo | esmalte azul | `[0.7, 1.0, 0.35, 0.0]` | 140 | — |

---

## La escena

### La isla

![Del alba a la noche](docs/arco-dia-noche.jpg)

La fuente flota sobre un mar de nubes. La isla sale de una grilla de celdas,
capa por capa: cada capa incluye las celdas que caen dentro de un radio que se
achica con la profundidad, deformado por ruido, y en cada fila las celdas
contiguas se funden en una sola caja. Pasto arriba, tierra debajo y roca que se
angosta en escalones hacia abajo: de costado se lee como un bloque de pasto de
Minecraft. De dos bordes caen cascadas de agua de verdad (refractan, y su
textura corre hacia abajo en cada cuadro), de la panza cuelgan puntas de
cristal encendido y alrededor flotan islotes más chicos que dan escala.

### Link

![Link tocando la ocarina](docs/link.jpg)

Link adulto de *Ocarina of Time*, hecho de **cubos que se pueden girar**
(`src/caja_orientada.rs`: en vez de girar la caja se gira el rayo, se resuelve
el test de las tres losas en el sistema de la caja y la normal vuelve al mundo
con los mismos ejes). Túnica, gorro largo, orejas de hylian, pelo rubio,
guanteletes, botas, el escudo hyliano y la Espada Maestra cruzados en la
espalda, y la Ocarina del Tiempo en la boca. La cara, la tela, el cuero y el
escudo son texturas pintadas píxel por píxel por código al arrancar.

Tiene **esqueleto**: raíz en los pies, torso, cabeza y un gorro que es una
cadena de cinco eslabones colgando de la coronilla, así que girar el torso
arrastra todo lo que cuelga de él. Los brazos se resuelven con **cinemática
inversa de dos huesos** para que las manos queden siempre sobre la ocarina. La
pose sale de la canción en cada cuadro: se mece a la mitad del compás, asiente
en cada tiempo, respira, y el gorro llega tarde a cada movimiento, como tela.

**Navi** revolotea alrededor de su cabeza con una luz propia que se mueve sobre
la túnica y el escudo, y cada ataque del arpa suelta una **nota** de la ocarina
que sube en espiral hacia la fuente, con los colores de los botones A (azul) y
C (amarillo) del juego.

### El arco: empieza al amanecer y termina de noche

El tema abre con el cielo rosa y oro sobre las nubes y la fuente apenas
visible. Sobre la mitad del tema el día se escurre —las estrellas aparecen, el
ambiente pasa de durazno a violeta, el sol se enfría hasta quedar en luna—
mientras las luces de la fuente **suben**. El clímax cae en noche cerrada, y
sobre la coda el horizonte se vuelve a encender y engancha con el arranque.

Todo eso cuelga de **un solo número**, `luz_del_dia`, que vale 1 al alba y 0 de
noche.

### El golpe

![El clímax: la columna de luz y la aurora](docs/golpe.jpg)

- **Anillos de luz en el agua**: en cada tiempo sale del pie del altar un
  anillo rosa que se abre hasta el borde de la piscina, iluminado y levantando
  el agua a su paso (una loma gaussiana que tuerce el reflejo). El uno del
  compás pega entero y los otros tres a un tercio, y la fuerza sigue a la
  energía del tema: un susurro en la intro, olas de luz en el coro.
- **La columna de luz**: cuando las voces se sostienen (el clímax), sube de la
  punta de la Trifuerza al cielo una columna con núcleo blanco y halo rosa
  translúcido, como cuando aparece el Hada Mayor, y late con el golpe.
- **La aurora boreal** vive toda la noche, tenue, y las voces la llevan a
  pleno; cada tiempo fuerte le da un latido. Son dos cortinas con el borde de
  abajo nítido y la cola deshilachada hacia arriba, rayos verticales que se
  corren solos y pliegues, verde en el filo, turquesa en el cuerpo y magenta
  arriba. Se refleja en el agua y en el mármol, y tiñe las nubes.
- Además: la ocarina se enciende con cada nota, Navi late, los anillos
  toroidales destellan, las hadas se deshacen con el arpa, estrellas fugaces y
  estelas cruzan en los tiempos fuertes, y el bloom, la niebla y los haces de
  luz respiran con la canción.

### La cámara

Un programa de planos que sigue la canción: abre **lejos y alta**, un plano
general de la isla sobre las nubes; baja y se acerca a medida que la música
crece; en el segundo 45 y en el 96 se va a **planos de Link** (de frente, con
la cara y la ocarina, y por encima del hombro, con la fuente delante), que caen
justo donde la vuelta de la órbita pone la cámara del lado correcto; en el
clímax queda en **contrapicado**, con la fuente recortada contra la aurora; y
con la coda vuelve a abrirse. Entre plano y plano interpola con Catmull-Rom,
así que pasa por ellos con velocidad en vez de frenar.

---

## Cómo está hecho

### El trazado

- La aritmética de vectores es propia (`src/vec3.rs`): `Vec3`, producto
  punto, producto cruz, normalización y los operadores. Sin librería de
  álgebra lineal — en un trazador de rayos eso es justamente la parte que
  hay que mostrar.
- Cuboides, **cubos orientados**, cilindros, planos recortados, esferas,
  triángulos y **toros**.
- El **toro** (`src/toro.rs`) es la única figura que no se resuelve con una
  cuadrática: sustituyendo el rayo en su ecuación implícita queda una
  **cuártica**, que se resuelve por Ferrari (deprimir, cúbica resolvente,
  dos cuadráticas) en `f64`, porque en `f32` los rayos rasantes se pierden y
  el anillo hierve. Tres de ellos rodean la Trifuerza y giran en precesión.
- Materiales con textura, mapa de relieve, reflexión y refracción con
  **Fresnel** (aproximación de Schlick) y sombras translúcidas: el agua y el
  cristal dejan pasar parte de la luz, así que el fondo de la piscina se
  ilumina a través del agua.
- **Skybox equirectangular generado por código**. Lo fijo (degradé,
  nebulosa, estrellas, luna) se hornea una vez; el **mar de nubes** también,
  evaluando el ruido en el punto donde cada dirección corta un plano de nubes
  muy por debajo de la isla, que es lo que les da perspectiva, pero su color
  se decide por rayo según la hora y la aurora. La aurora, las estrellas
  fugaces y el amanecer se calculan por rayo.
- **BVH** construido con la heurística de área, **rearmado en cada cuadro**
  con las cajas de ese instante, y un **BVH propio adentro de cada grupo
  estático** (la isla, el techo, la piscina). El árbol de sombras va aparte y
  solo lleva lo que puede tapar la luz.
- Los rayos se podan por contribución acumulada, no por profundidad: un camino
  que ya no puede cambiar ni un nivel de 255 no se sigue.
- **Oclusión ambiental** con dos rayos por impacto en el hemisferio
  ponderado por el coseno, con el largo de cada rayo sorteado (eso da la
  caída con la distancia gratis).
- **Reflejos rugosos**: el rayo reflejado se desvía según la rugosidad del
  material, y el acumulador temporal promedia.
- **Antialiasing temporal** con reproyección y recorte de vecindad.

### El post-procesado (GLSL 330, en luz lineal)

- **Bloom por cadena de seis mips**, el método de *Call of Duty: Advanced
  Warfare*: filtro de 13 muestras al bajar, carpa de 3×3 al subir.
- Curva de tono **AgX**, que sube al blanco sin torcer el tono de los colores
  saturados, que es todo el problema en una escena de rosas y cianes.
- Niebla cuadrática por profundidad (que deja casi limpio al cielo, para que
  no lave la aurora), god rays.
- Profundidad de campo, estelas anamórficas, aberración cromática, viñeta,
  grano y cierre a formato ancho.
- **CAS** (realce adaptativo por contraste) para recuperar el filo que pierde
  el estirado de 400×300 a 800×600.

### La sincronización

`analizar_audio.py` saca bandas de frecuencia, onsets, beats, secciones y
chroma a 30 fps y los deja en un JSON. El trazador lo lee al arrancar y lo
indexa por tiempo; no sabe que existe librosa. Todo lo que se mueve es
**función del segundo de la canción**, no un estado que avanza cuadro a
cuadro: el mismo segundo da siempre la misma imagen, se dibuje a 5 o a 40
cuadros por segundo.

### Las texturas

![Las texturas de archivo](docs/texturas.jpg)

Las de la fuente son procedurales, 512×512, generadas por
`generar_texturas_cueva.py` con NumPy y semilla fija. Las de la isla (pasto,
tierra) y las de Link (cara, tela, cuero, pelo, escudo hyliano) se pintan en
Rust al arrancar, píxel por píxel, con `TextureImage::pintada`.

```bash
python3 generar_texturas_cueva.py            # regenerar las texturas
python3 analizar_audio.py assets/music/fairy_fountain.mp3 \
    --salida fairy_fountain_sync.json        # regenerar el análisis (pide librosa)
```

---

## Rendimiento

Medido en un MacBook Air M3, trazando a 400×300 y estirando a 800×600, con
`--bench` (el mínimo de cinco pasadas por momento):

| | antes | ahora |
|---|---|---|
| Trazado, media de los cinco momentos | 36.6 ms (27 fps) | **23.4 ms (43 fps)** |
| Peor plano (Link de cerca, de noche) | — | ~35 ms |

Y eso con la escena **tres veces más grande** (de 34 a 126 objetos, de 8 a 10
luces). Las notebooks sin ventilador varían un 30% según la temperatura, así
que las decisiones se tomaron **contando instrucciones ejecutadas**
(`/usr/bin/time -l`), que no dependen del calor. Con el perfilador de macOS
(`sample`) se encontraron tres cosas:

1. **El árbol se armaba una sola vez**, y todo lo que se mueve tenía que
   declarar una caja que cubriera su recorrido entero: las estelas del arpa
   llevaban cajas de dieciséis unidades de lado y ningún rayo que pasara cerca
   de la fuente podía descartarlas. Recorrer el árbol era más de la mitad del
   cuadro. Ahora se rearma en cada cuadro (un centenar de cajas, microsegundos)
   y cada grupo que se mueve ajusta su caja a donde quedó: **−15%**.
2. **Los reflejos débiles no preguntan por la sombra.** Un rebote que llega al
   píxel con menos del 35% de peso (el reflejo borroso del piso) ilumina su
   impacto sin tirar rayos de sombra; los rebotes fuertes (el agua de
   costado, la obsidiana) la siguen calculando. La diferencia no se ve y era
   la quinta parte del cuadro: **−14%**.
3. **Grupos estáticos con árbol propio** y **una caja por hada y por mota**:
   las motas de polvo estaban en cuatro cajas de seis por cinco unidades, y
   cualquier rayo que cruzara la fuente probaba sus quince esferas. **−7%**.

La palanca grande sigue siendo la resolución de trazado (`RENDER_W` /
`RENDER_H` en `src/main.rs`), no el post-procesado: el bloom, la niebla y el
resto corren en la GPU y cuestan casi lo mismo a cualquier resolución.

---

## Música

`assets/music/fairy_fountain.mp3` es el **Great Fairy's Fountain Theme** de
*The Legend of Zelda 25th Anniversary Soundtrack*. Los derechos son de
Nintendo; está acá solo para que el proyecto se pueda correr tal cual. *The
Legend of Zelda*, Link, Navi y la Trifuerza son de Nintendo; este es un
trabajo de facultad sin fines de lucro.

Si el archivo no está, la escena corre igual con un reloj interno.
