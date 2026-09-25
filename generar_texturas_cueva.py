"""
Genera las seis texturas del diorama de cueva magica (estilo Great Fairy
Fountain de Zelda) y las deja en resources/textures/.

Uso:
  python3 generar_texturas_cueva.py [--tam 512]

Solo NumPy y Pillow. La semilla esta fija (42) para que las texturas
salgan IGUALES cada vez que se corre: son parte de la escena, no un
efecto aleatorio.

POR QUE 512 Y NO 64. Las texturas eran de 64x64 y el trazador las
muestrea con filtro BILINEAL (`texture.rs`), asi que cada texel se
estiraba sobre decenas de pixeles de pantalla y lo que se veia no era
piedra: era una acuarela. Un cubo de la piscina mide seis unidades de
mundo y llena media pantalla; con 64 texeles eso son menos de diez
texeles por cada cien pixeles. A 512 hay ocho veces mas detalle por eje
y el costo en tiempo de trazado es EXACTAMENTE EL MISMO (bilineal lee
cuatro texeles, de a 64 o de a 512 da igual); lo unico que sube es la
memoria, de 16 KB a 1 MB por textura, que sobre seis texturas son 6 MB.

QUE CAMBIA ADEMAS DEL TAMANO. Subir el tamano solo no alcanzaba: casi
todos los patrones estaban escritos EN PIXELES (grietas de un pixel de
ancho, destellos sueltos al 5%, molduras de dos pixeles de borde). A 512
una grieta de un pixel es invisible, y peor: el detalle de un solo texel
TITILA, porque el cuadro se traza a 400x300 y un texel que no llega a
cubrir un pixel entra y sale del muestreo cuando la camara se mueve. Asi
que los patrones se rehicieron en unidades RELATIVAS al tamano, con el
detalle puesto en frecuencias medias y los bordes suavizados, que es lo
que sobrevive al minificado sin centellear.

Las tecnicas son las clasicas del ruido procedural:
  - TURBULENCIA (fBm del valor absoluto) para el marmol: la veta sale de
    sin(x + turbulencia), que es la receta de Perlin de 1985 y sigue
    siendo la que mejor se ve;
  - DEFORMACION DEL DOMINIO (domain warping, de Inigo Quilez): evaluar
    el ruido en un punto que a su vez movio otro ruido, fbm(p + fbm(p)).
    Es lo que convierte manchas redondas en algo que fluye;
  - VORONOI (ruido de Worley) para las caras del cristal y las grietas
    de la piedra, con la diferencia F2 - F1, que da las aristas;
  - y para el agua, una red de CAUSTICAS: el mismo voronoi invertido y
    elevado, que es como se ve la luz concentrada por una superficie
    ondulada.

Las seis son deliberadamente etereas, no fotorrealistas: la paleta es
rosa pastel, cyan suave, teal, dorado y morado profundo.
"""

import argparse
import os

import numpy as np
from PIL import Image

TAM = 512
SALIDA = os.path.join("resources", "textures")


# ============================================================
#  UTILIDADES DE RUIDO
# ============================================================

def ruido_valor(tam, celdas, rng):
    """Ruido de valor suave: una grilla aleatoria de `celdas` x `celdas`
    interpolada hasta `tam` x `tam`.

    Se repite SIN COSTURA: la grilla se cierra sobre si misma (el vecino
    de la ultima celda es la primera), asi que el borde derecho empalma
    con el izquierdo. Importa porque estas texturas se repiten en mosaico
    sobre las caras de los cubos, y una costura visible delata el truco.
    """
    celdas = max(2, int(celdas))
    grilla = rng.random((celdas, celdas))

    t = np.arange(tam) * celdas / tam
    i = np.floor(t).astype(int) % celdas
    f = t - np.floor(t)
    # Suavizado de quintico grado (Perlin 2002): 6t^5 - 15t^4 + 10t^3.
    # Frente al smoothstep cubico tiene tambien la SEGUNDA derivada nula
    # en los extremos, y eso quita las bandas horizontales y verticales
    # tenues que el cubico deja sobre los bordes de celda. A 64 texeles
    # no se veian; a 512, sobre una pared lisa, si.
    f = f * f * f * (f * (f * 6 - 15) + 10)
    i1 = (i + 1) % celdas

    a = grilla[np.ix_(i, i)]
    b = grilla[np.ix_(i, i1)]
    c = grilla[np.ix_(i1, i)]
    d = grilla[np.ix_(i1, i1)]

    arriba = a + (b - a) * f[None, :]
    abajo = c + (d - c) * f[None, :]
    return arriba + (abajo - arriba) * f[:, None]


def fbm(tam, rng, octavas=5, celdas=4, persistencia=0.5):
    """Fractal Brownian motion: varias capas de ruido, cada una con el
    doble de frecuencia y la mitad de peso. Devuelve valores en [-1, 1].

    Las celdas se DUPLICAN por octava (4, 8, 16, 32, 64...) en vez de
    partirse a la mitad, que es lo que hacia la version vieja al reves:
    asi la frecuencia mas alta que se genera esta atada al tamano de la
    textura y no a un numero de pixeles fijo.
    """
    total = np.zeros((tam, tam))
    peso = 1.0
    suma = 0.0
    for _ in range(octavas):
        total += ruido_valor(tam, celdas, rng) * peso
        suma += peso
        peso *= persistencia
        celdas *= 2
    return (total / suma) * 2 - 1


def turbulencia(tam, rng, octavas=5, celdas=4, persistencia=0.5):
    """Como el fBm pero sumando el VALOR ABSOLUTO de cada octava.

    La diferencia se ve: el fBm es suave en todas partes, y la
    turbulencia tiene PLIEGUES, porque el valor absoluto quiebra el ruido
    donde cruza el cero. Esos pliegues son lo que hace que la veta del
    marmol parezca una veta y no una mancha.
    """
    total = np.zeros((tam, tam))
    peso = 1.0
    suma = 0.0
    for _ in range(octavas):
        total += np.abs(ruido_valor(tam, celdas, rng) * 2 - 1) * peso
        suma += peso
        peso *= persistencia
        celdas *= 2
    return total / suma


def deformar(campo, dx, dy, fuerza):
    """DEFORMACION DEL DOMINIO: devuelve `campo` leido en (x + dx*f,
    y + dy*f) en vez de en (x, y), con envoltura toroidal.

    Es el truco de Inigo Quilez: en lugar de sumar dos ruidos, se usa uno
    para MOVER el punto donde se evalua el otro. Dos ruidos sumados
    siguen viendose como dos ruidos; uno deformado por el otro se ve como
    algo que fluye, y es de donde sale el aspecto de marmol pulido y de
    roca erosionada.

    El desplazamiento se hace con indices enteros (`np.take`), que es
    barato y, sobre un campo que ya es suave, indistinguible de
    interpolar.
    """
    tam = campo.shape[0]
    y, x = np.mgrid[0:tam, 0:tam]
    xs = np.mod(np.rint(x + dx * fuerza).astype(int), tam)
    ys = np.mod(np.rint(y + dy * fuerza).astype(int), tam)
    return campo[ys, xs]


def voronoi(tam, n_semillas, rng):
    """Ruido de Worley toroidal. Devuelve (F1, F2, celda): la distancia a
    la semilla mas cercana, a la segunda, y cual es la mas cercana.

    Todo normalizado a fraccion del tamano, asi el patron se ve igual sea
    cual sea la resolucion. F2 - F1 es casi cero justo sobre la frontera
    entre dos celdas: de ahi salen las aristas del cristal y las grietas
    de la piedra.
    """
    semillas = rng.random((n_semillas, 2))
    y, x = np.mgrid[0:tam, 0:tam]
    x = x / tam
    y = y / tam

    dx = np.abs(x[:, :, None] - semillas[None, None, :, 0])
    dy = np.abs(y[:, :, None] - semillas[None, None, :, 1])
    # Envoltura toroidal: la distancia por el borde puede ser mas corta.
    dx = np.minimum(dx, 1.0 - dx)
    dy = np.minimum(dy, 1.0 - dy)
    dist = np.sqrt(dx * dx + dy * dy)

    orden = np.argsort(dist, axis=2)
    f1 = np.take_along_axis(dist, orden[:, :, :1], axis=2)[:, :, 0]
    f2 = np.take_along_axis(dist, orden[:, :, 1:2], axis=2)[:, :, 0]
    return f1, f2, orden[:, :, 0]


def lienzo(color, tam=TAM):
    """Imagen RGB flotante llena de un color."""
    return np.ones((tam, tam, 3), dtype=np.float64) * np.array(color, dtype=np.float64)


def mezclar(img, color, cantidad):
    """Lleva `img` hacia `color` segun `cantidad` (0 a 1, una matriz)."""
    color = np.array(color, dtype=np.float64)[None, None, :]
    return img + (color - img) * cantidad[:, :, None]


def guardar(nombre, rgb):
    """Recorta a 0..255, agrega alfa opaco y escribe el PNG."""
    rgb = np.clip(rgb, 0, 255).astype(np.uint8)
    alfa = np.full((rgb.shape[0], rgb.shape[1], 1), 255, dtype=np.uint8)
    rgba = np.concatenate([rgb, alfa], axis=2)
    ruta = os.path.join(SALIDA, nombre)
    Image.fromarray(rgba, mode="RGBA").save(ruta)
    print(f"  {ruta}  ({rgb.shape[1]}x{rgb.shape[0]})")


def coords(tam=TAM):
    """Coordenadas normalizadas (0 a 1) de cada pixel, como dos matrices."""
    y, x = np.mgrid[0:tam, 0:tam]
    return x / tam, y / tam


def glints(tam, densidad, rng, radio_px):
    """Un campo de destellos REDONDOS de `radio_px` pixeles, no de un
    texel suelto.

    Un destello de un solo texel es lo peor que se le puede poner a una
    textura que se va a minificar: cuando el texel no alcanza a cubrir un
    pixel de pantalla, entra y sale del muestreo bilineal segun donde caiga
    la camara, y el resultado es una superficie que HIERVE. Con un radio
    de dos o tres pixeles y caida suave, el filtro siempre agarra algo del
    destello y el brillo queda quieto.
    """
    campo = np.zeros((tam, tam))
    n = int(tam * tam * densidad)
    if n <= 0:
        return campo
    cx = rng.integers(0, tam, size=n)
    cy = rng.integers(0, tam, size=n)
    fuerza = rng.random(n) ** 2

    r = int(np.ceil(radio_px))
    for dy in range(-r, r + 1):
        for dx in range(-r, r + 1):
            d = np.hypot(dx, dy) / radio_px
            if d > 1.0:
                continue
            caida = (1.0 - d) ** 2
            np.maximum.at(campo, ((cy + dy) % tam, (cx + dx) % tam), fuerza * caida)
    return campo


# ============================================================
#  1. PIEDRA DE CUEVA MAGICA
# ============================================================

def stone_cave(rng, tam):
    """La roca del piso y las paredes: oscura, humeda, con grietas.

    Tres capas: una base de fBm deformado (el bulto grande de la roca),
    una red de grietas sacada de las fronteras de un voronoi, y un
    picoteado fino que le da grano a la superficie.
    """
    img = lienzo((35, 35, 50), tam)

    # El bulto grande, deformado: sin la deformacion la roca se ve como
    # manchas de camuflaje; con ella, como algo erosionado.
    base = fbm(tam, rng, octavas=5, celdas=3)
    warp_x = fbm(tam, rng, octavas=3, celdas=4)
    warp_y = fbm(tam, rng, octavas=3, celdas=4)
    base = deformar(base, warp_x, warp_y, tam * 0.09)
    img += (base * 14)[:, :, None]

    # GRIETAS: las fronteras del voronoi, que forman una red conectada
    # como la de una roca partida de verdad (y no las caminatas
    # aleatorias sueltas de antes, que a 512 se perdian). El campo se
    # deforma para que las grietas no sean rectas.
    f1, f2, _ = voronoi(tam, 26, rng)
    frontera = f2 - f1
    frontera = deformar(frontera, warp_x, warp_y, tam * 0.035)
    # Ancho en fraccion del tamano: a cualquier resolucion la grieta mide
    # lo mismo respecto de la textura.
    grieta = np.clip(1.0 - frontera / 0.018, 0.0, 1.0) ** 1.5
    img = mezclar(img, (20, 20, 34), grieta * 0.85)

    # Grano fino: el picoteado de la piedra. Frecuencia alta pero
    # amplitud baja, que es lo que se puede tener sin que centellee.
    img += (fbm(tam, rng, octavas=3, celdas=48) * 5)[:, :, None]

    # HUMEDAD: parches donde la roca refleja la luz magica. Son manchas,
    # no pixeles sueltos: un fBm de frecuencia baja recortado por arriba.
    mojado = np.clip((fbm(tam, rng, octavas=4, celdas=6) - 0.28) / 0.45, 0, 1)
    # Se apaga adentro de las grietas: el agua escurre por la cara, no
    # por el fondo de la fisura.
    mojado *= 1.0 - grieta
    img = mezclar(img, (58, 60, 88), mojado * 0.7)

    return img


# ============================================================
#  2. CRISTAL MAGICO ROSA
# ============================================================

def crystal(rng, tam):
    """El cuarzo rosa de los racimos: caras planas, aristas encendidas y
    fracturas internas."""
    base = np.array((220, 150, 200), dtype=np.float64)

    # Las CARAS: un voronoi de pocas semillas, cada una con su tono. Que
    # sean pocas es la gracia: un cristal tiene cuatro o cinco caras
    # grandes, no cincuenta manchitas.
    f1, f2, celda = voronoi(tam, 7, rng)
    tonos = rng.uniform(-26, 26, size=(7, 3))
    img = base[None, None, :] + tonos[celda]

    # Volumen adentro de cada cara: oscura en el centro de la celda,
    # clara hacia la arista, que es como se ve un prisma iluminado por
    # dentro.
    t = np.clip(f1 / np.maximum(f1 + f2, 1e-6) * 2, 0, 1)
    img += (np.array((-22.0, -22.0, -22.0))[None, None, :]) * (1 - t)[:, :, None]

    # ARISTAS encendidas, con un nucleo fino y un halo ancho: asi la
    # arista se lee como un filo que atrapa la luz y no como una linea
    # dibujada encima.
    frontera = f2 - f1
    nucleo = np.clip(1.0 - frontera / 0.010, 0, 1) ** 0.8
    halo = np.clip(1.0 - frontera / 0.055, 0, 1) ** 2.2
    img = mezclar(img, (248, 226, 244), halo * 0.35)
    img = mezclar(img, (255, 244, 252), nucleo * 0.9)

    # FRACTURAS INTERNAS: las plumas blancas que tiene el cuarzo por
    # dentro. Turbulencia deformada y recortada muy arriba, asi quedan
    # unos pocos trazos finos y no una nube.
    pluma = turbulencia(tam, rng, octavas=5, celdas=5)
    wx = fbm(tam, rng, octavas=3, celdas=3)
    wy = fbm(tam, rng, octavas=3, celdas=3)
    pluma = deformar(pluma, wx, wy, tam * 0.13)
    pluma = np.clip((pluma - 0.62) / 0.30, 0, 1) ** 1.6
    img = mezclar(img, (255, 235, 248), pluma * 0.55)

    # Polvo finisimo, para que la cara no sea un plano de color.
    img += (fbm(tam, rng, octavas=3, celdas=40) * 4)[:, :, None]

    return img


# ============================================================
#  3. OBSIDIANA MAGICA
# ============================================================

def obsidian(rng, tam):
    """El vidrio volcanico del pedestal: casi negro, con el flujo
    congelado adentro y el reflejo vitreo de la fractura concoidea."""
    img = lienzo((15, 8, 25), tam)

    # LAS BANDAS DE FLUJO: la obsidiana es lava que se enfrio de golpe, y
    # lo que se ve adentro son las capas del flujo estiradas. Eso es una
    # veta tipo marmol pero MUY estirada en un eje: sin(x*k + turbulencia).
    x, y = coords(tam)
    turb = turbulencia(tam, rng, octavas=5, celdas=4)
    # El eje va en diagonal (x + y) para que el flujo no sea horizontal
    # y se lea igual en cualquier cara del cubo.
    flujo = np.sin((x + y) * np.pi * 7.0 + turb * 5.0)
    veta = np.clip(flujo, 0, 1) ** 2.5
    img = mezclar(img, (26, 16, 44), veta * 0.75)

    # FRACTURA CONCOIDEA: la obsidiana se parte en conchas, superficies
    # curvas cuyos arcos atrapan la luz. Voronoi otra vez, pero usando F1
    # como un anillado en vez de la frontera.
    #
    # APENAS visible, y solo el ARCO EXTERIOR de cada concha. El primer
    # intento tenia el anillado a frecuencia alta y todos los arcos
    # marcados por igual, y eso no se lee como vidrio partido sino como
    # los anillos de un tronco: un blanco de tiro repetido. Una concha
    # real es una sola superficie curva, asi que aca se pinta un unico
    # borde por celda (el recorte alto sobre F1) y con un tercio de la
    # fuerza.
    f1, f2, _ = voronoi(tam, 14, rng)
    concha = np.clip((f1 / np.maximum(f1 + f2, 1e-6) * 2 - 0.55) / 0.45, 0, 1) ** 2.2
    # Deformada con el mismo flujo, asi el borde de la concha sigue la
    # veta en vez de cortarla.
    wx = fbm(tam, rng, octavas=3, celdas=4)
    wy = fbm(tam, rng, octavas=3, celdas=4)
    concha = deformar(concha, wx, wy, tam * 0.05)
    img = mezclar(img, (36, 25, 60), concha * 0.30)

    # El brillo vitreo: destellos de radio pequeno pero REDONDOS, con el
    # violeta que le pone la luz de las hadas.
    destello = glints(tam, 0.0012, rng, radio_px=max(2.0, tam / 220))
    img = mezclar(img, (74, 54, 118), np.clip(destello, 0, 1) * 0.8)

    img += (fbm(tam, rng, octavas=3, celdas=32) * 3)[:, :, None]

    return img


# ============================================================
#  4. AGUA MAGICA DE LA FUENTE
# ============================================================

def water_fairy(rng, tam):
    """La superficie del agua vista desde arriba: la RED DE CAUSTICAS que
    dibuja la luz al atravesar una superficie ondulada.

    La caustica es donde la superficie del agua CONCENTRA la luz, y esas
    zonas de concentracion forman una red de filamentos brillantes con
    celdas oscuras adentro. Justo la topologia de un voronoi invertido:
    se toma la distancia a la frontera entre celdas y se la eleva, asi la
    frontera queda encendida y el centro de la celda apagado. Sobre eso
    van dos capas a distinta escala, porque la caustica real tiene
    filamentos gruesos y finos a la vez.

    (La version vieja eran tres senos multiplicados. Cerraba el ciclo sin
    costura, que es lo que se buscaba, pero la red de una caustica no es
    una grilla de lobulos: es irregular, y con senos siempre se leia como
    tela escocesa.)
    """
    base = np.array((14, 58, 78), dtype=np.float64)
    profundo = np.array((8, 38, 56), dtype=np.float64)
    filamento = np.array((120, 225, 245), dtype=np.float64)

    img = lienzo(base, tam)

    # El ondulado que deforma todo: es lo que hace que la red no sea de
    # poligonos rectos sino de filamentos curvos.
    wx = fbm(tam, rng, octavas=4, celdas=4)
    wy = fbm(tam, rng, octavas=4, celdas=4)

    def red(n_semillas, ancho, fuerza_warp):
        f1, f2, _ = voronoi(tam, n_semillas, rng)
        frontera = deformar(f2 - f1, wx, wy, tam * fuerza_warp)
        return np.clip(1.0 - frontera / ancho, 0, 1) ** 1.7

    # Capa gruesa (los filamentos principales) y capa fina encima.
    gruesa = red(16, 0.055, 0.055)
    fina = red(42, 0.024, 0.035)

    # Las celdas, entre filamento y filamento, son mas OSCURAS que la
    # base: la luz que se concentro en la caustica salio de algun lado.
    img = mezclar(img, profundo, np.clip(1.0 - gruesa * 1.4, 0, 1) * 0.55)

    # Y los filamentos, encendidos. La suma va en modo "pantalla" para
    # que donde se cruzan dos filamentos el brillo no se recorte plano.
    luz = np.clip(gruesa * 0.85 + fina * 0.55, 0, 1.4)
    luz = 1.0 - np.exp(-luz * 1.6)
    img = mezclar(img, filamento, luz * 0.85)

    # El agua profunda tiene un gradiente lentisimo de temperatura: unas
    # zonas mas verdes, otras mas azules. Es lo que evita que la piscina
    # se lea como un color plano con un patron encima.
    temp = fbm(tam, rng, octavas=3, celdas=3)
    img[:, :, 1] += temp * 9
    img[:, :, 2] += -temp * 7

    return img


# ============================================================
#  5. ORO / MINERAL SAGRADO
# ============================================================

def gold_triforce(rng, tam):
    """El oro de las molduras: metal MARTILLADO, con las abolladuras del
    martillo y el pulido direccional entre ellas.

    (Antes eran pepitas de oro sueltas sobre piedra, que a 64 se leian
    como mineral en bruto. En la fuente las molduras son oro trabajado:
    una pieza entera, no una veta.)
    """
    oro = np.array((208, 170, 72), dtype=np.float64)
    img = lienzo(oro, tam)

    # EL ORO ES CASI LISO. Lo que lo hace leerse como metal no es tener
    # mucha textura sino tener MUY POCA y bien orientada.
    #
    # Dos intentos anteriores pusieron abolladuras de voronoi como motivo
    # principal, con celdas grandes primero y chicas despues, y las dos
    # veces la celda GANO: el oro salia como panal de abejas o como piel
    # de lagarto. El problema no era el tamano de la celda, era que el
    # voronoi fuera el patron dominante. Un voronoi siempre se lee como
    # voronoi si es lo mas fuerte del cuadro.
    #
    # Asi que el orden se invierte: primero el brillo direccional, que es
    # el 80% de lo que hace metal a un metal, y las abolladuras encima
    # apenas insinuadas.

    # 1. VARIACION LARGA DE BRILLO. Una pieza de metal curvada refleja
    #    distinto en cada zona, y eso son manchas grandes y suaves de
    #    claro y oscuro, sin ningun detalle adentro.
    largo = fbm(tam, rng, octavas=3, celdas=2)
    img += (largo * 34)[:, :, None]

    # 2. PULIDO DIRECCIONAL: las rayitas finas del trapo y la lima, que
    #    en una pieza trabajada corren TODAS en el mismo sentido. Ruido
    #    aplastado sobre un eje. Es lo que lo delata como metal, porque
    #    ningun material natural se raya asi.
    rayas = ruido_valor(tam, max(8, tam // 2), rng) * 2 - 1
    for _ in range(4):
        rayas = (rayas + np.roll(rayas, 1, axis=1) + np.roll(rayas, -1, axis=1)) / 3.0
    img += (rayas * 30)[:, :, None]

    # 3. LOS GOLPES DE MARTILLO, al final y flojos: solo el reborde de
    #    cada abolladura, que es lo unico que se ve en una pieza batida
    #    de verdad bajo luz difusa.
    f1, f2, _ = voronoi(tam, 46, rng)
    hueco = np.clip(f1 / np.maximum(f1 + f2, 1e-6) * 2, 0, 1)
    desp = max(2, tam // 150)
    sombra = np.roll(hueco, desp, axis=0) - hueco
    img += (sombra * 55)[:, :, None]

    # El calido y el frio del oro: en las zonas hundidas se acumula la
    # patina (mas rojiza y oscura) y en las crestas el reflejo (mas
    # blanco). Sin esta separacion el oro se lee como plastico amarillo,
    # y esto SI puede ir fuerte porque no tiene forma propia: sigue la
    # variacion larga, que no dibuja ningun motivo.
    frio = np.clip((largo + 0.15) / 0.8, 0, 1)
    img = mezclar(img, (150, 106, 34), (1.0 - frio) * 0.40)
    img = mezclar(img, (255, 236, 168), np.clip((frio - 0.62) / 0.38, 0, 1) ** 1.4 * 0.55)

    # Y el chispazo del metal pulido.
    img = mezclar(img, (255, 250, 222), np.clip(glints(tam, 0.0006, rng, max(2.0, tam / 260)), 0, 1) * 0.6)

    return img


# ============================================================
#  6. PIEDRA MAGICA TEAL (MARMOL DE LA FUENTE)
# ============================================================

def fairy_marble(rng, tam):
    """El marmol cyan/teal de la Great Fairy Fountain: NO es gris.

    El marmol se hace con la receta de Perlin: una onda seno a la que se
    le suma TURBULENCIA adentro del argumento. La onda sola daria bandas
    rectas; la turbulencia las retuerce, y como la turbulencia tiene
    pliegues (por el valor absoluto), la veta sale con los filos y los
    nudos que tiene el marmol de verdad.

    Encima va una segunda familia de vetas mas finas y en otra direccion,
    porque una sola familia se lee como madera.
    """
    base = np.array((92, 162, 157), dtype=np.float64)
    veta_oscura = np.array((52, 108, 106), dtype=np.float64)
    veta_clara = np.array((168, 225, 218), dtype=np.float64)

    # LA VETA SALE DEL CONTORNO CERO DE UN RUIDO, no de una onda seno.
    #
    # La receta clasica de Perlin (umbralar sin(x + turbulencia)) tiene un
    # problema que en esta textura se ve de inmediato: el seno es
    # PERIODICO, asi que las vetas salen todas a la misma distancia una de
    # otra y con el mismo grosor. Se probo y el marmol quedaba como
    # corteza de arbol o pana: rayas parejas. El marmol de verdad tiene
    # tres vetas juntas, un claro grande, una veta sola que se bifurca.
    #
    # Lo que da eso es tomar un ruido `n` y quedarse con lo que esta CERCA
    # DE CERO: `1 - |n|` elevado a una potencia alta. El contorno cero de
    # un ruido es una curva irregular, que se bifurca y se cierra sola,
    # con tramos separados por distancias distintas; la potencia decide el
    # grosor. Es el mismo truco del ruido "ridged" de los terrenos, usado
    # al reves: en vez de crestas de montana, grietas.
    #
    # Y el ruido se evalua sobre un dominio ESTIRADO en un eje (el fBm se
    # muestrea con celdas distintas en x y en y a traves del deformado),
    # que es lo que le da a la veta una direccion dominante sin volverla
    # periodica.
    n = fbm(tam, rng, octavas=5, celdas=3)
    wx = fbm(tam, rng, octavas=4, celdas=5)
    wy = fbm(tam, rng, octavas=4, celdas=5)
    # El deformado es ANISOTROPO (mucho en x, poco en y): las vetas se
    # estiran y quedan mayormente verticales, como en una losa cortada.
    n = deformar(n, wx, wy * 0.25, tam * 0.16)

    # `1 - |n|` vale 1 justo sobre el contorno cero y cae a los lados. La
    # potencia 14 lo deja en una linea fina.
    oscura = np.clip(1.0 - np.abs(n) / 0.22, 0, 1) ** 2.2

    # Una segunda familia, mas fina y con otra semilla, para que haya
    # vetas de dos grosores. Con una sola el marmol se lee dibujado.
    n2 = fbm(tam, rng, octavas=6, celdas=6)
    n2 = deformar(n2, wy, wx * 0.3, tam * 0.10)
    oscura = np.maximum(oscura, np.clip(1.0 - np.abs(n2) / 0.10, 0, 1) ** 2.6 * 0.6)

    img = lienzo(base, tam)

    # Las vetas OSCURAS son finas y definidas (es la impureza metida en
    # la fisura), las CLARAS son anchas y difusas (es la calcita). Tratar
    # a las dos igual es lo que hace que un marmol procedural se vea
    # pintado; esta asimetria es casi todo el efecto.
    #
    # La clara NO sale del contorno cero sino del ruido crudo: es una
    # nube, no una linea, y va lejos de donde estan las vetas oscuras.
    clara = np.clip((n - 0.18) / 0.55, 0, 1) ** 1.5

    img = mezclar(img, veta_clara, clara * 0.40)
    img = mezclar(img, veta_oscura, oscura * 0.85)

    # El grano de la piedra pulida, apenas.
    img += (fbm(tam, rng, octavas=4, celdas=30) * 4)[:, :, None]

    # CRISTALES MICROSCOPICOS: el marmol de la fuente tiene que
    # centellear un poco, es magico. Redondos y chicos, por lo mismo que
    # en la obsidiana.
    img = mezclar(img, (206, 248, 240),
                  np.clip(glints(tam, 0.0009, rng, max(2.0, tam / 240)), 0, 1) * 0.55)

    # LA MOLDURA TALLADA del borde: al repetirse la textura, cada cubo
    # queda enmarcado. El ancho va en FRACCION del tamano (2.5%) y con
    # caida suave: a 512 un borde de dos pixeles no existe, y uno duro
    # aliasea.
    ancho = max(2.0, tam * 0.025)
    dist_borde = np.minimum(
        np.minimum(np.arange(tam)[:, None], tam - 1 - np.arange(tam)[:, None]),
        np.minimum(np.arange(tam)[None, :], tam - 1 - np.arange(tam)[None, :]),
    ).astype(np.float64)
    marco = np.clip(1.0 - dist_borde / ancho, 0, 1)
    # Un bisel: la parte de afuera de la moldura esta en sombra y la de
    # adentro atrapa la luz, asi el marco tiene relieve y no es una raya.
    bisel = np.clip(1.0 - np.abs(dist_borde - ancho) / (ancho * 0.9), 0, 1)
    img = mezclar(img, (48, 100, 98), (marco ** 1.4) * 0.75)
    img = mezclar(img, (142, 205, 198), bisel * 0.35)

    return img


# ============================================================
#  MAIN
# ============================================================

def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--tam", type=int, default=TAM,
                    help="lado de las texturas en pixeles (por defecto 512)")
    args = ap.parse_args()
    tam = args.tam

    rng = np.random.default_rng(42)
    os.makedirs(SALIDA, exist_ok=True)

    print(f"Generando texturas de la cueva magica a {tam}x{tam}:")
    guardar("stone_cave.png", stone_cave(rng, tam))
    guardar("crystal.png", crystal(rng, tam))
    guardar("obsidian.png", obsidian(rng, tam))
    guardar("water_fairy.png", water_fairy(rng, tam))
    guardar("gold_triforce.png", gold_triforce(rng, tam))
    guardar("fairy_marble.png", fairy_marble(rng, tam))
    print("Listo.")


if __name__ == "__main__":
    main()
