//! La Great Fairy Fountain de Ocarina of Time, trazada con rayos.
//!
//! Una piscina cuadrada de marmol teal con seis columnas alrededor y un
//! techo con el centro abierto, agua cyan que brilla desde abajo, molduras
//! de oro, y en el medio del agua un pedestal de obsidiana con la Triforce
//! rodeada de hadas: esferitas emisivas que la musica hace respirar. Todo
//! adentro de una cueva oscura que apenas se ve. Lo que lo hace leerse
//! como la fuente es la suma de cuatro cosas: el teal del marmol contra el
//! rosa de las hadas, el agua espejo, niebla azul oscura, y bloom generoso
//! que difunde todo lo que brilla.
//!
//! Lo que hace el trazador por cada impacto, en orden:
//!   - NORMAL MAPPING: la normal geometrica se inclina con un mapa de
//!     relieve procedural (`texture.rs`), asi la piedra tiene grano, el
//!     marmol vetas y el oro esta martillado sin un triangulo de mas;
//!   - luces puntuales de COLOR con atenuacion por distancia y sombras
//!     TRANSLUCIDAS: el agua y el cristal dejan pasar parte de la luz;
//!   - FRESNEL (Schlick) repartiendo entre reflexion y refraccion segun
//!     el angulo: el agua es espejo de lejos y agua de cerca;
//!   - reflexion y refraccion recursivas, podadas por contribucion;
//!   - y lo que no pega en nada va al SKYBOX: un cielo equirectangular
//!     generado por codigo con estrellas, nebulosa y luna (`cielo.rs`).
//!
//! EL TEMA EMPIEZA AL AMANECER Y TERMINA DE NOCHE, y esa es la estructura
//! del cuadro entero. Abre con el cielo rosa y oro, el ambiente calido, la
//! bruma de la manana y la fuente apenas visible: no puede competir con el
//! cielo, y que no compita es el punto. Sobre la mitad del tema el dia se
//! escurre —las estrellas aparecen, el ambiente pasa de durazno a violeta,
//! el sol se enfria hasta ser la luna— mientras las luces de la fuente
//! SUBEN. Es un fundido cruzado: la fuente no cambia tanto de brillo como
//! de protagonismo. El coro y el climax caen en noche cerrada, con la
//! fuente siendo lo unico que ilumina. Sobre la coda el horizonte se
//! vuelve a encender y engancha con el arranque, asi que el loop es un dia
//! entero por vuelta.
//!
//! Todo eso cuelga de UN numero, `luz_del_dia` (`sync.rs`), que vale 1 al
//! alba y 0 de noche. De el salen la luz ambiente, el resplandor del
//! cielo, el color y la densidad de la niebla, el color y la potencia del
//! sol/luna, cuanto brilla la fuente, la fuerza del bloom, su umbral y la
//! de las estelas. El cielo ademas GIRA a lo largo del tema, asi que el
//! astro cruza de izquierda a derecha y su luz lo sigue.
//!
//! El post-procesado en la GPU (`resources/shaders/glsl330/`) trabaja en
//! luz lineal: bloom, niebla, tinte, curva de tono (AgX), god rays,
//! caleidoscopio, y al final profundidad de campo, aberracion cromatica,
//! vinieta y grano.
//!
//! El BLOOM es una cadena de seis mips (el metodo de Call of Duty:
//! Advanced Warfare): se reduce la imagen a la mitad seis veces y despues
//! se vuelve a subir mezclando cada nivel sobre el anterior, asi que el
//! halo es la suma de seis escalas de desenfoque a la vez. Ver
//! `bloom_down.fs` y `bloom_up.fs`.
//!
//! Como se acelera el trazado, que es de lo que depende que esto corra en
//! tiempo real: las primitivas se agrupan en CAJAS alineadas a los ejes
//! (`grupo_acotado.rs`) y los grupos cuelgan de un BVH construido con la
//! heuristica de area (`bvh.rs`). Medido sobre esta escena, pasar de
//! esferas acotantes a cajas bajo el cuadro de 53 a 35 milisegundos sin
//! cambiar un solo pixel, y el corte temprano del rayo de sombra lo dejo
//! en 32.
//!
//! El reparto de responsabilidades:
//!   - `audio.rs`      de donde sale el segundo en el que estamos;
//!   - `sync.rs`       como tiene que estar la escena en ese segundo;
//!   - `animacion.rs`  escribir eso sobre las hadas, el agua y las luces;
//!   - `cielo.rs`      el skybox;
//!   - `bvh.rs`        el arbol que evita probar todos los objetos;
//!   - `cube.rs`       los cuboides de la fuente, la cueva y los cristales;
//!                     `cylinder.rs` las columnas, `plane.rs` el agua,
//!                     `sphere.rs` las hadas, `triangle.rs` la Triforce y
//!                     las puntas de los cristales, y `toro.rs` los anillos
//!                     que la rodean (la unica figura que pide resolver una
//!                     cuartica);
//!   - este archivo    construir la escena, trazarla y armar el pipeline
//!                     de post-procesado en la GPU.
//!
//! Modos de linea de comandos:
//!   - (nada)          la escena en vivo con la musica;
//!   - `--bench`       traza cinco cuadros crudos, los mide y los vuelca;
//!   - `--foto <s>`    abre la ventana, traza el segundo `s` con TODO el
//!                     post-procesado y guarda `foto_t<s>.png`;
//!   - `--sin-taa`     apaga el antialiasing temporal (para compararlo);
//!   - `--taa`         banco de pruebas del antialiasing temporal: vuelca
//!                     el cuadro crudo, el acumulado y la referencia 2x2
//!                     del mismo instante, con la camara en movimiento;
//!   - `--sync [paso]` imprime lo que el analisis pide, segundo a segundo.
//!
//! EL POST-PROCESADO NO CORRE EN LA CPU. El trazador produce color y
//! profundidad y nada mas; el bloom, la niebla, la vinieta, el tinte y la
//! compresion de rango son tres shaders de fragmentos en
//! `resources/shaders/glsl330/`. La diferencia no es de estilo: el bloom en
//! CPU cuesta proporcionalmente al radio (un halo de 35 pixeles son ~55 ms
//! por cuadro y crecen si se lo agranda), y en la GPU un radio enorme sale
//! igual de barato que uno chico porque el desenfoque va separado en dos
//! ejes, a mitad de resolucion, sobre miles de nucleos.

mod animacion;
mod audio;
mod azar;
mod bvh;
mod camera;
mod cielo;
mod cube;
mod framebuffer;
mod grupo_acotado;
mod light;
mod material;
mod ray_intersect;
mod sync;
mod texture;
mod vec3;
/// Del modulo de cilindros la cueva usa solo el vertical; el orientado
/// (los laseres del escenario anterior) se queda en el engine, y sin el
/// `allow` seria un warning.
#[allow(dead_code)]
mod cylinder;
mod plane;
mod sphere;
mod toro;
mod triangle;

use animacion::EscenaViva;
use audio::RelojEscena;
use bvh::Bvh;
use camera::Camera;
use cielo::Cielo;
use cube::Cube;
use cylinder::Cylinder;
use framebuffer::Framebuffer;
use grupo_acotado::GrupoAcotado;
use light::Light;
use material::Material;
use crate::vec3::{cross, dot, normalize, Vec3};
use plane::{Limite, Plane};
use ray_intersect::{Intersect, RayIntersect};
use raylib::prelude::*;
use rayon::prelude::*;
use sphere::Sphere;
use sync::SceneParams;
use std::f32::consts::PI;
use std::sync::Arc;
use texture::{Texture, TextureImage};
#[allow(unused_imports)]
use toro::Toro;
use triangle::Triangle;

const WIDTH: usize = 800;
const HEIGHT: usize = 600;

/// Negro. Lo que se ve donde el rayo no le pega a nada lo pone el skybox,
/// asi que esto solo importa mientras el framebuffer esta vacio.
const BACKGROUND: Color = Color::new(0, 0, 0, 255);

/// Luz ambiente de la NOCHE, por canal. La del amanecer es
/// `AMBIENTE_DIA`, y `ambiente_de` interpola entre las dos con la hora.
///
/// CASI CERO, apenas para que nada sea negro total. Con luces puntuales y
/// sin rebotes, el ambiente es lo unico que ilumina las caras que no miran
/// a ninguna luz; subirlo es comodo porque no deja nada negro, pero aca lo
/// oscuro es el tema. La cueva se lee por las islas de color que rompen la
/// oscuridad, y si el ambiente las levanta del piso se lava todo y queda
/// un diagrama gris.
///
/// Violeta oscuro, a proposito. Es el brillo MINIMO de cualquier superficie,
/// y en la Fairy Fountain las sombras tienen color, no son negras: un
/// violeta profundo que sugiere la cueva sin comerse el contraste de las
/// luces. Con 0.05 todo lo que no recibia luz directa era negro.
///
/// Subido de (0.10, 0.08, 0.16) cuando el post-procesado paso a lineal:
/// la curva de tono en lineal hunde las sombras mucho mas que antes, y con
/// el ambiente viejo las caras que no miran a ninguna luz (el techo por
/// abajo, el dorso de las columnas) salian negro pleno y el cuadro quedaba
/// duro. Con este, en la sombra queda un violeta que todavia se lee.
const AMBIENTE_NOCHE: [f32; 3] = [0.20, 0.16, 0.30];

/// Y la luz ambiente del AMANECER, con la que arranca el tema.
///
/// Es lo que de verdad distingue el dia de la noche, y no las luces
/// puntuales. Una luz puntual ilumina lo que mira hacia ella; el ambiente
/// ilumina TODO, incluido el dorso de las columnas y el techo por abajo,
/// que de noche son negro violaceo. Subirlo es lo que hace que la escena
/// se lea como un lugar abierto a la luz en vez de como una cueva.
///
/// Tres veces mas alto que el de la noche y con el sesgo dado vuelta: ahi
/// manda el azul (0.30 contra 0.20 de rojo) porque la sombra de una cueva
/// es violeta; aca manda el rojo, porque la luz del alba es calida y lo
/// que rebota del aire a esa hora es rosa y oro. El cambio de SESGO se
/// lee mas que el de cantidad: entre los dos extremos la escena no pasa
/// de oscura a clara, pasa de violeta a durazno.
const AMBIENTE_DIA: [f32; 3] = [0.62, 0.50, 0.44];

/// Y el ambiente de LA AURORA: el que tiene la cueva cuando el cielo se
/// enciende sobre el clima sostenido de las voces (ver `SyncData::swell`).
///
/// LA AURORA SOLA NO ALCANZABA, y esto se midio: con la cortina encendida
/// en el cielo, comparando el mismo cuadro con y sin ella, la aurora tocaba
/// el 0.8% de los pixeles. El motivo es la escena, no la aurora: del cielo
/// se ven los rincones entre columna y columna, porque el borde de roca del
/// fondo y el techo tapan casi todo. Diecinueve segundos de voces
/// sostenidas no pueden colgar del 0.8% del cuadro.
///
/// Asi que lo que entra en la cueva es la LUZ del cielo: el ambiente se
/// corre del violeta de la noche a un verde azulado, que es el color de la
/// base de la cortina. El ambiente ilumina TODO, incluido el dorso de las
/// columnas y el techo por abajo, asi que el cuadro entero cambia de clima
/// sin que se encienda una sola luz nueva.
///
/// Es un cambio de SESGO y no de cantidad, igual que entre la noche y el
/// alba: la escena no pasa de oscura a clara, pasa de violeta a verde. Eso
/// es lo que se puede hacer durante diecinueve segundos sin cansar.
const AMBIENTE_AURORA: [f32; 3] = [0.17, 0.35, 0.33];

/// El ambiente de este instante: entre los tres de arriba segun la hora y
/// segun cuanto esten sosteniendo las voces.
fn ambiente_de(luz_del_dia: f32, swell: f32) -> [f32; 3] {
    let d = luz_del_dia.clamp(0.0, 1.0);
    // La aurora no existe de dia (ver `Cielo::aurora`), asi que su tinte se
    // apaga con la noche igual que ella.
    let a = swell.clamp(0.0, 1.0) * (1.0 - d) * 0.75;
    let mezcla = |noche: f32, dia: f32, aurora: f32| {
        let hora = noche + (dia - noche) * d;
        hora + (aurora - hora) * a
    };
    [
        mezcla(AMBIENTE_NOCHE[0], AMBIENTE_DIA[0], AMBIENTE_AURORA[0]),
        mezcla(AMBIENTE_NOCHE[1], AMBIENTE_DIA[1], AMBIENTE_AURORA[1]),
        mezcla(AMBIENTE_NOCHE[2], AMBIENTE_DIA[2], AMBIENTE_AURORA[2]),
    ]
}

/// De donde salen las texturas de la fuente. Las genera
/// `generar_texturas_cueva.py`; la ruta es relativa a la raiz del proyecto,
/// que es desde donde corre `cargo run`.
const TEXTURAS: &str = "resources/textures";

/// El tema: el Great Fairy's Fountain Theme (25th Anniversary Soundtrack).
/// El mp3 original tiene el nombre largo con apostrofe y espacios; se lo
/// copio a `fairy_fountain.mp3` para que la ruta sea sana. Es relativa al
/// directorio desde donde se corre el programa, que con `cargo run` es la
/// raiz del proyecto. Si el archivo no esta, la escena corre igual con un
/// reloj interno. Se prueban en orden y gana la primera que exista, asi da
/// igual si el mp3 quedo en la raiz o adentro de assets.
///
/// El analisis que le corresponde sale de:
///   python3 analizar_audio.py assets/music/fairy_fountain.mp3 \
///       --salida fairy_fountain_sync.json
const AUDIO_PATHS: [&str; 3] = [
    "fairy_fountain.mp3",
    "assets/music/fairy_fountain.mp3",
    "assets/fairy_fountain.mp3",
];

/// Cuanto tiene que aportar un rayo de reflexion o refraccion al color final
/// para que valga la pena tirarlo.
///
/// No es el peso de ESE rebote: es el peso ACUMULADO de todo el camino. Un
/// reflejo del piso pesa 0.85, pero un reflejo del piso que despues rebota
/// en la pared pesa 0.85 x 0.05 = 0.042, y uno mas alla de eso 0.002, que en
/// un canal de 0..255 es medio nivel: invisible.
///
/// Sin esto, con cinco rebotes y dos rayos por impacto (reflexion Y
/// refraccion), un solo pixel puede llegar a disparar treinta y dos rayos, y
/// treinta de esos no cambian ni un nivel del resultado. Cortar por
/// contribucion poda el arbol por donde no se ve y deja intactos los caminos
/// que si importan: piso -> cristal -> anillo, que es la toma.
const MIN_CONTRIBUCION: f32 = 0.03;

/// CUANTAS LUCES RECUERDA EL CACHE DE SOMBRA. Ver `Sombras`.
///
/// La escena tiene cinco luces con multiplicador mas las de ambiente; con
/// dieciseis ranuras sobra, y si algun dia hubiera mas, las de mas alla de
/// la ranura ultima simplemente no usan cache y andan igual.
const LUCES_CACHE: usize = 16;

/// EL CACHE DE SOMBRA: quien tapo la ultima vez.
///
/// Una ranura por luz. `usize::MAX` es
/// "nadie". Vive por FILA de pixeles, que es la unidad en la que se reparte
/// el trazado entre nucleos, asi que es de un solo hilo por construccion y
/// no hace falta ningun candado.
#[derive(Clone)]
struct Sombras {
    /// Quien tapo esta luz en el pixel anterior.
    luces: [usize; LUCES_CACHE],
}

impl Sombras {
    fn vacio() -> Sombras {
        Sombras { luces: [usize::MAX; LUCES_CACHE] }
    }
}

/// Aporte minimo de una luz para que valga la pena tirarle el rayo de
/// sombra. En una escala de 0 a 1 por canal, 0.004 es un nivel de 255:
/// por debajo de eso la luz es literalmente invisible.
const MIN_APORTE_LUZ: f32 = 0.004;

/// La escala de las olas con la que se dibujan las causticas del fondo.
///
/// Es la misma que la de la superficie (`ripple_scale` del plano del agua,
/// 3.0): con otra, los filamentos del fondo no caerian donde estan las
/// olas de arriba.
const CAUSTICA_ESCALA: f32 = 3.0;

/// El color de la luz que la superficie concentra en el fondo.
///
/// Cyan clarisimo, casi blanco. Es luz, no pintura: va SUMADA, asi que lo
/// que decide cuanto se ve es la potencia con la que se suma y no su
/// saturacion, y un color muy saturado sumado sobre el teal del fondo daria
/// un verde acido que no es lo que hace el agua.
const CAUSTICA_COLOR: Color = Color::new(200, 245, 255, 255);

/// Cuantos rayos de OCLUSION AMBIENTAL se tiran por impacto primario.
///
/// La oclusion ambiental es cuanto cielo ve un punto. Un rincon ve poco y
/// queda oscuro; una superficie despejada ve todo y queda clara. Sin ella
/// la luz ambiente llega igual a todas partes y nada se APOYA en nada: las
/// columnas no tocan el piso, el pedestal flota sobre el agua y la escena
/// se lee como un collage de objetos puestos uno delante de otro.
///
/// DOS, y el numero salio de medir, no de elegir. Con cuatro el cuadro
/// pasa de 33.9 a 42.7 ms (+26%) y con dos a 38.0 (+12%), y en la imagen
/// FINAL las dos versiones son indistinguibles: el acumulador temporal
/// promedia cuatro cuadros, asi que dos rayos por cuadro ya son ocho
/// muestras efectivas, y eso alcanza de sobra para una senial de
/// frecuencia baja. Pagar cuatro es pagar el doble por un ruido que el
/// acumulador se iba a comer igual.
///
/// Ese es tambien el motivo por el que aca funciona el muestreo
/// estocastico y en las sombras suaves no (ver el comentario largo del
/// bucle de luces): la oclusion cambia DESPACIO a lo largo de una
/// superficie, y una rampa suave muestreada poco y promediada es la
/// rampa. El borde de una sombra es un escalon, y un escalon muestreado
/// poco es ruido.
const AO_RAYOS: usize = 2;

/// Hasta donde mira la oclusion ambiental, en unidades del mundo.
///
/// Es el parametro que decide la ESCALA del efecto y hay que elegirlo
/// contra el tamano de la escena, no a ojo: con un radio muy chico solo se
/// oscurece la linea exacta donde dos cosas se tocan y parece suciedad; con
/// uno muy grande la oclusion deja de describir rincones y se convierte en
/// un sombreado general que apaga la escena.
///
/// 1.6 es del orden del ancho de una columna (0.7) y de la profundidad de
/// la piscina: oscurece el encuentro de las columnas con el piso, las
/// esquinas de la piscina, el hueco debajo de las losas del techo y el pie
/// del pedestal, que son exactamente los lugares donde el ojo busca la
/// pista de que una cosa se apoya sobre otra.
const AO_RADIO: f32 = 1.6;

/// Cuanto llega a oscurecer, de 0 a 1.
///
/// No llega a 1 a proposito: con oclusion total los rincones quedan negro
/// pleno y el efecto se nota como efecto. A 0.85 el rincon mas cerrado
/// conserva un 15% de su luz ambiente.
const AO_FUERZA: f32 = 0.85;

/// Rebotes de reflexion y refraccion.
///
/// Tres alcanzan: un rayo entra a un cristal, sale, y todavia puede
/// rebotar una vez en el agua o en la obsidiana. Con cinco la cueva se veia
/// igual y costaba casi el doble, porque cada cristal dispara reflexion Y
/// refraccion en cada rebote.
const MAX_DEPTH: u32 = 3;

/// Cuantos pixeles traza el raytracer, y son SIEMPRE estos.
///
/// No es la resolucion de la ventana: la ventana es de WIDTH x HEIGHT y el
/// cuadro trazado se estira hasta llenarla con filtro bilineal. Trazar por
/// debajo de la pantalla es lo que hace que la escena entre en tiempo real
/// con cinco rebotes; el estirado no se nota porque la imagen es de por si
/// blanda (casi todo son luces difusas y reflejos, no bordes finos).
///
/// FIJOS y no ajustados en vivo. Antes esto se movia solo entre diez
/// escalones para sostener un piso de cuadros por segundo, y el remedio era
/// peor que la enfermedad: la resolucion cambiaba de golpe en medio de la
/// cancion, cada cambio obligaba a rehacer el buffer y la textura, y la
/// imagen respiraba de nitida a blanda sin que eso tuviera nada que ver con
/// la musica. Con un valor fijo la escena se ve siempre igual y los cuadros
/// por segundo son los que son.
///
/// Para trazar a la resolucion de la ventana, poner 800 y 600 aca. La
/// cueva va a la mitad: son quinientos cubos con refraccion y a 600 x 450
/// quedaba en tres cuadros por segundo. El post-procesado (bloom, niebla,
/// tinte) corre en la GPU a su propia resolucion y no se entera.
const RENDER_W: u32 = 400;
const RENDER_H: u32 = 300;

// ---------- Utilidades de color ----------
// raylib usa u8 por canal, pero el sombreado se hace en f32.
// Se multiplica en flotante y se recorta al final.

fn scale_color(color: Color, factor: f32) -> Color {
    Color::new(
        (color.r as f32 * factor).clamp(0.0, 255.0) as u8,
        (color.g as f32 * factor).clamp(0.0, 255.0) as u8,
        (color.b as f32 * factor).clamp(0.0, 255.0) as u8,
        255,
    )
}

/// Como `scale_color` pero con un peso distinto por canal. Hace falta para
/// el ambiente, que no es un gris parejo sino un violeta muy oscuro.
fn tint_color(color: Color, factor: [f32; 3]) -> Color {
    Color::new(
        (color.r as f32 * factor[0]).clamp(0.0, 255.0) as u8,
        (color.g as f32 * factor[1]).clamp(0.0, 255.0) as u8,
        (color.b as f32 * factor[2]).clamp(0.0, 255.0) as u8,
        255,
    )
}

/// Un color dado por canal en 0..1, como lo escribe la consigna de la cueva.
fn color_f(r: f32, g: f32, b: f32) -> Color {
    Color::new(
        (r * 255.0).clamp(0.0, 255.0) as u8,
        (g * 255.0).clamp(0.0, 255.0) as u8,
        (b * 255.0).clamp(0.0, 255.0) as u8,
        255,
    )
}

fn add_colors(a: Color, b: Color) -> Color {
    Color::new(
        a.r.saturating_add(b.r),
        a.g.saturating_add(b.g),
        a.b.saturating_add(b.b),
        255,
    )
}

/// Arma una cara rectangular con dos triangulos.
///
/// Este escenario no arma caras planas, pero la utilidad se queda: es parte
/// del engine y borrarla no estaba en el pedido.
///
/// Los cuatro vertices van en orden ANTIHORARIO visto DESDE AFUERA de la
/// pieza. Eso importa porque el Triangle de esta escena no voltea la normal
/// hacia el rayo: la saca de cross(b - a, c - a) y punto, asi que el orden
/// de los vertices ES la cara. Al revez, la normal apunta hacia adentro, el
/// difuso da cero y la cara se apaga.
#[allow(dead_code)]
fn quad(a: Vec3, b: Vec3, c: Vec3, d: Vec3, material: &Material) -> [Triangle; 2] {
    let face = |x: Vec3, y: Vec3, z: Vec3| Triangle {
        a: x,
        b: y,
        c: z,
        uv_a: None,
        uv_b: None,
        uv_c: None,
        material: material.clone(),
    };

    [face(a, b, c), face(a, c, d)]
}

/// A que "profundidad" se anota el cielo en el depth buffer.
///
/// El cielo esta infinitamente lejos, pero anotarlo asi (255 en el alpha) lo
/// deja ahogado en niebla en cuanto la cancion sube la densidad, y las
/// estrellas desaparecen justo cuando la escena mas las necesita. Con 22
/// unidades queda apenas mas lejos que el borde de la cueva (que esta a
/// unas 20 desde la camara): se vela como lo que esta al fondo, no como el
/// infinito.
const CIELO_PROFUNDIDAD: f32 = 22.0;

/// Reflectancia de Fresnel por la aproximacion de Schlick.
///
/// `cos` es el coseno entre la normal y la direccion hacia el ojo, y `ior`
/// el indice de refraccion del material. De frente casi todo entra (el agua
/// refleja el 2%); a angulo rasante casi todo rebota (100%). Es lo que
/// hace que el agua se vea como espejo desde lejos y como agua desde
/// arriba, y sin esto los dos pesos de reflexion y refraccion son
/// constantes y el agua se lee como vidrio pintado.
fn schlick(cos: f32, ior: f32) -> f32 {
    let f0 = ((ior - 1.0) / (ior + 1.0)).powi(2);
    f0 + (1.0 - f0) * (1.0 - cos.clamp(0.0, 1.0)).powi(5)
}

/// Los pesos de reflexion y refraccion de un impacto, ya pasados por
/// Fresnel: `(reflexion, refraccion)`.
///
/// Con refraccion, lo que Fresnel manda a reflejar se lo quita a lo que
/// atraviesa: a angulo rasante el agua es espejo y no deja ver el fondo.
/// Sin refraccion (marmol, obsidiana), la reflexion crece hacia el rasante
/// pero en proporcion a lo pulido que sea el material: la obsidiana llega
/// a espejo, el marmol apenas se abrillanta.
fn pesos_fresnel(material: &Material, cos: f32) -> (f32, f32) {
    let kr = material.albedo[2];
    let kt = material.albedo[3];

    if kt > 0.0 {
        let f = schlick(cos, material.refractive_index.max(1.0));
        ((kr + kt * f).min(1.0), kt * (1.0 - f))
    } else if kr > 0.0 {
        let f = schlick(cos, 1.5);
        (kr + (1.0 - kr) * f * (kr / (kr + 0.3)), 0.0)
    } else {
        (0.0, 0.0)
    }
}

/// La normal con el relieve del material aplicado, si lo tiene.
///
/// El mapa da la normal en espacio tangente (x a lo largo de u, y a lo
/// largo de v, z hacia afuera); aca se arma una base tangente a partir de
/// la normal geometrica y se la lleva al mundo. La base se elige con el eje
/// del mundo menos alineado a la normal: para las caras de los cubos y el
/// piso eso coincide con la direccion de las UV, y para las esferas y los
/// cilindros da una tangente horizontal, que es lo que se espera.
fn normal_con_relieve(intersect: &Intersect) -> Vec3 {
    let n = intersect.normal;
    let Some((mapa, repeticiones)) = &intersect.material.relieve else {
        return n;
    };

    let tn = mapa.sample_normal(intersect.u * repeticiones, intersect.v * repeticiones);

    let ayuda = if n.y.abs() < 0.9 {
        Vec3::new(0.0, 1.0, 0.0)
    } else {
        Vec3::new(1.0, 0.0, 0.0)
    };
    let tangente = normalize(&cross(&ayuda, &n));
    let bitangente = cross(&n, &tangente);

    normalize(&(tangente * tn.x + bitangente * tn.y + n * tn.z))
}

/// Refleja un vector sobre una normal: R = I - 2(I·N)N
fn reflect(incident: &Vec3, normal: &Vec3) -> Vec3 {
    incident - normal * (2.0 * dot(incident, normal))
}

/// Refracta un vector segun la ley de Snell.
/// eta_i es el medio de donde viene el rayo (aire = 1.0) y eta_t
/// el medio al que entra. Si hay reflexion total interna devuelve
/// el rayo reflejado en vez del refractado.
fn refract(incident: &Vec3, normal: &Vec3, eta_t: f32) -> Vec3 {
    let mut cos_i = dot(incident, normal).clamp(-1.0, 1.0);
    let mut eta_i = 1.0;
    let mut eta_t = eta_t;
    let mut n = *normal;

    if cos_i > 0.0 {
        // El rayo viene desde ADENTRO del objeto: la normal apunta al
        // lado equivocado, asi que se invierte y se cambian los medios.
        n = -n;
        std::mem::swap(&mut eta_i, &mut eta_t);
        cos_i = -cos_i;
    }

    let eta = eta_i / eta_t;
    // sin^2 del rayo transmitido, por Snell: sin_t = eta * sin_i
    let sin2_t = eta * eta * (1.0 - cos_i * cos_i);

    if sin2_t > 1.0 {
        // Reflexion total interna: el rayo no logra salir del medio.
        return reflect(incident, &n);
    }

    let cos_t = (1.0 - sin2_t).sqrt();
    incident * eta + n * (eta * (-cos_i) - cos_t)
}

// ---------- Trazado ----------

/// Devuelve el color del pixel y la distancia al impacto mas cercano
/// (f32::MAX si no hubo). Esa distancia alimenta el depth buffer que usa la
/// niebla del post-procesado; en las llamadas recursivas de reflexion y
/// refraccion se ignora, porque la profundidad que importa es la del PRIMER
/// impacto que ve la camara.
fn cast_ray(
    origin: &Vec3,
    direction: &Vec3,
    objects: &[Box<dyn RayIntersect + Send + Sync>],
    cielo: &Cielo,
    // Los dos arboles: el de todos los objetos, para los rayos que buscan
    // que se ve, y el de los que pueden tapar la luz, para los de sombra.
    // Ver `bvh.rs`.
    arbol: &Bvh,
    arbol_sombras: &Bvh,
    lights: &[Light],
    depth: u32,
    max_depth: u32,
    // Cuanto de este rayo termina llegando al pixel: 1.0 para el rayo
    // primario, y se va multiplicando por el albedo en cada rebote.
    peso: f32,
    // La luz ambiente DE ESTE CUADRO, por canal. Ya no es una constante:
    // la hora del dia la mueve entre `AMBIENTE_DIA` y `AMBIENTE_NOCHE`
    // (ver `luz_del_dia` en `sync.rs`). Es lo que hace que el amanecer se
    // sienta amanecer: lo que separa el dia de la noche no son las luces
    // puntuales sino cuanta luz hay en el AIRE, y eso es exactamente esto.
    ambiente: [f32; 3],
    // La fase de las olas en este cuadro, para que las causticas del fondo
    // de la piscina se muevan con la superficie de arriba. Ver
    // `plane::caustica`.
    fase_agua: f32,
    // La semilla de azar de ESTE rayo: distinta por pixel y por cuadro.
    // De ella salen el desvio del reflejo y el punto de la luz al que
    // apunta el rayo de sombra. Ver `azar.rs`.
    semilla: u32,
    // Quien tapo la luz la ultima vez, por luz. Ver `Sombras`.
    sombras: &mut Sombras,
) -> (Color, f32) {
    // Corte de la recursion: sin esto dos espejos enfrentados
    // se llaman para siempre. El limite es un parametro porque
    // mientras se mueve la camara conviene bajarlo.
    if depth > max_depth {
        return (cielo.color(direction), CIELO_PROFUNDIDAD);
    }

    // El arbol entrega los candidatos de cerca hacia lejos y se le va
    // devolviendo la distancia del mejor impacto, con lo que poda solo
    // todo lo que quedo detras.
    let mut zbuffer = f32::INFINITY;
    let mut intersect = Intersect::empty();

    arbol.recorrer(origin, direction, f32::INFINITY, |i, limite| {
        let hit = objects[i].ray_intersect(origin, direction);
        if hit.is_intersecting && hit.distance < zbuffer {
            zbuffer = hit.distance;
            intersect = hit;
            return zbuffer;
        }
        limite
    });

    if !intersect.is_intersecting {
        return (cielo.color(direction), CIELO_PROFUNDIDAD);
    }

    let view_dir = normalize(&(origin - intersect.point));

    // La normal que ve la luz: la geometrica inclinada por el relieve del
    // material. Los rayos de sombra y los rebotes se desplazan sobre la
    // GEOMETRICA, que es la unica que garantiza salir de la superficie.
    let normal = normal_con_relieve(&intersect);
    let (peso_kr, peso_kt) = pesos_fresnel(intersect.material, dot(&view_dir, &normal));

    // El color base ya no sale del material directo: lo da la textura
    // segun las UV del impacto. Se calcula una sola vez, no por luz.
    let base_color = intersect.material.texture.get_color(intersect.u, intersect.v);

    // --- OCLUSION AMBIENTAL ---
    //
    // Solo en el impacto PRIMARIO (`depth == 0`). En los rebotes se saltea:
    // multiplicaria el costo por cada nivel de reflexion y refraccion para
    // corregir la luz ambiente de algo que ya se ve reflejado y al 25% de
    // su peso.
    //
    // EL LARGO DE CADA RAYO SE SORTEA, y ese es el truco que da la caida
    // con la distancia sin pagarla. Lo que se querria es que un oclusor
    // pegado oscurezca mucho y uno lejano poco, pero para eso haria falta
    // saber A QUE DISTANCIA golpeo cada rayo, y la prueba barata del arbol
    // (`occluded`) solo contesta si o no, sin distancia; pedir la distancia
    // obliga a la interseccion completa, que arma el impacto y clona el
    // material.
    //
    // Sorteando el largo maximo de cada rayo entre 0 y `AO_RADIO`, la
    // probabilidad de que un oclusor a distancia d bloquee un rayo es la
    // probabilidad de que ese rayo haya salido mas largo que d, o sea
    // 1 - d/AO_RADIO. La caida lineal aparece sola en el promedio, con
    // pruebas de si o no y sin una sola division extra.
    let mut ao = 0.0f32;
    if depth == 0 {
        let origen_ao = intersect.point + intersect.normal * 1e-3;
        for k in 0..AO_RAYOS {
            let s = azar::revolver(semilla ^ (k as u32).wrapping_mul(0x9e37_79b9) ^ 0x00a0_00a0);

            // Direccion PONDERADA POR EL COSENO: sumarle a la normal un
            // punto de la esfera unitaria y normalizar da exactamente eso,
            // y es lo que corresponde porque lo que se esta integrando
            // lleva el coseno adentro. Repartir las direcciones parejas
            // por el hemisferio en vez de asi mide lo mismo con un 30% mas
            // de error para el mismo numero de rayos.
            let sesgo = intersect.normal + azar::en_esfera(s);
            // Si el punto sorteado cae casi opuesto a la normal la suma se
            // va a cero y la direccion queda indefinida; ahi se usa la
            // normal, que es el limite correcto.
            let dir = if sesgo.magnitude_squared() > 1e-6 {
                normalize(&sesgo)
            } else {
                intersect.normal
            };

            let largo = AO_RADIO * azar::uniforme(s ^ 0x5eed_1234);

            // SE PROBO EL CACHE DE LAS SOMBRAS TAMBIEN ACA Y SALE PEOR:
            // 39.7, 38.1 y 39.8 ms contra 37.4, 37.7 y 38.1. El truco vive
            // de la coherencia, y estos rayos no la tienen: van a
            // direcciones sorteadas del hemisferio, asi que el que tapo al
            // anterior casi nunca tapa al siguiente y lo unico que queda es
            // un test de geometria regalado por rayo, dos por impacto.
            let mut tapado = false;
            arbol_sombras.recorrer(&origen_ao, &dir, largo, |i, limite| {
                if objects[i].occluded(&origen_ao, &dir, limite) {
                    tapado = true;
                    0.0
                } else {
                    limite
                }
            });
            if tapado {
                ao += 1.0;
            }
        }
        // CURVA DE CONTRASTE antes de aplicarla. La fraccion cruda de
        // rayos tapados es casi siempre chica —la escena es una plaza
        // abierta, no un interior de rincones— y medida sobre el cuadro da
        // 0.11 de media, que despues de la curva de tono no se ve. La raiz
        // levanta justamente la parte baja del rango (0.25 pasa a 0.5) sin
        // tocar los extremos, asi que el rincon cerrado queda igual de
        // cerrado y lo que gana es la zona de contacto, que es la que tiene
        // que leerse.
        ao = (ao / AO_RAYOS as f32).sqrt() * AO_FUERZA;
    }

    // --- Iluminacion ---
    // Cada luz aporta su propio difuso y especular, con su propia
    // sombra. Los aportes se van sumando: por eso donde se cruzan dos
    // luces de colores distintos el color se mezcla.
    // La iluminacion arranca del ambiente, no de cero: sobre ese piso se
    // van sumando las luces.
    // La oclusion multiplica SOLO EL AMBIENTE, no la luz directa. Es lo
    // correcto y ademas es lo que la hace ver bien: el ambiente representa
    // la luz que llega rebotada de todas partes, y es justamente esa la que
    // un rincon no recibe. La luz directa de una lampara no le importa el
    // rincon: o la ve o esta en sombra, y de eso ya se encargan los rayos
    // de sombra. Aplicandola a todo, la oclusion se lee como mugre pintada
    // encima en vez de como falta de luz.
    let visible = 1.0 - ao;
    let mut lit = tint_color(
        scale_color(base_color, intersect.material.albedo[0]),
        [ambiente[0] * visible, ambiente[1] * visible, ambiente[2] * visible],
    );

    // --- CAUSTICAS ---
    //
    // El fondo de la piscina recibe la luz que la superficie de arriba
    // CONCENTRA: una red de filamentos brillantes que se mueve con las
    // olas. Se suma al ambiente y no a una luz en particular porque no
    // viene de una sola: el agua concentra todo lo que le llega, y en esta
    // escena eso es la luz cenital, la del agua y el cielo a la vez.
    //
    // Se calcula con la MISMA suma de olas que inclina la superficie
    // (`plane::pendiente_de_las_olas`), asi que los filamentos caen donde
    // la ola que los produce esta plana. Con dos animaciones distintas se
    // leerian como dos capas superpuestas.
    if intersect.material.causticas > 0.0 {
        let c = plane::caustica(
            intersect.point.x,
            intersect.point.z,
            CAUSTICA_ESCALA,
            fase_agua,
        );
        lit = add_colors(
            lit,
            scale_color(CAUSTICA_COLOR, c * intersect.material.causticas),
        );
    }

    for (numero_luz, light) in lights.iter().enumerate() {
        // CORTE TEMPRANO POR APORTE.
        //
        // Con la atenuacion por distancia, una luz lejana no cambia ni un
        // nivel de 255 en este punto, pero igual se le tiraba su rayo de
        // sombra, que es lo mas caro que hay por impacto. Preguntando
        // primero cuanto puede llegar a aportar, las luces chicas (las
        // antorchas, de alcance 4) dejan de costar en toda la escena menos
        // en el rincon donde se las ve. Es lo que hace que agregar luces
        // de ambiente no se pague en cuadros por segundo.
        let distancia = (light.position - intersect.point).magnitude();
        if light.intensity * light.atenuacion(distancia) < MIN_APORTE_LUZ {
            continue;
        }

        let light_dir = normalize(&(light.position - intersect.point));

        // Si la luz esta DETRAS de la superficie no aporta nada: el difuso
        // sale cero y el especular seria un brillo imposible, de una luz
        // que no se ve desde aca. Cortar antes del rayo de sombra ahorra
        // recorrer los ~140 objetos de la escena, y con ocho luces eso es
        // la mayor parte del trabajo de cada impacto.
        let diffuse_intensity = dot(&normal, &light_dir);
        if diffuse_intensity <= 0.0 {
            continue;
        }

        // --- SOMBRAS ---
        // El origen se desplaza un poquito sobre la normal para que el
        // rayo no se choque con la superficie de la que sale (shadow acne).
        let shadow_origin = intersect.point + intersect.normal * 1e-3;

        let shadow_distance = distancia;

        // SE PROBO HACER LAS SOMBRAS SUAVES Y SE DESCARTO. Queda anotado
        // para que no se reintente a ciegas.
        //
        // La idea era la de siempre: darle un radio a cada luz y apuntar el
        // rayo de sombra a un punto al azar de esa esfera en cada cuadro,
        // dejando que el acumulador temporal promediara los aciertos y los
        // fallos hasta formar la penumbra. No cuesta ni un rayo mas, asi
        // que parecia gratis.
        //
        // No lo es, por dos razones que solo aparecen al medir:
        //
        //   - CUESTA UN 19% DEL CUADRO (34.5 a 41.2 ms, minimo de cinco
        //     corridas). Un rayo de sombra bloqueado corta apenas encuentra
        //     al primero que tapa; uno que pasa tiene que recorrer el arbol
        //     entero para poder afirmarlo. Al jitterear, en la penumbra la
        //     mitad de los rayos dejan de bloquearse y pasan a pagar el
        //     recorrido completo, y ademas los rayos vecinos dejan de ir en
        //     direcciones parecidas y el recorrido pierde coherencia.
        //
        //   - Y NO SE VE COMO PENUMBRA SINO COMO GRANO. El acumulador pesa
        //     el cuadro nuevo 0.25, o sea que promedia cuatro muestras
        //     efectivas, no las catorce del modo foto. Cuatro muestras de
        //     una visibilidad que solo puede valer 0 o 1 dan cinco niveles
        //     posibles. Restando las dos imagenes, la diferencia no se
        //     concentra en bandas en el borde de las sombras —que es lo
        //     que habria que ver— sino repartida como sal y pimienta por
        //     todas las superficies iluminadas.
        //
        // Para que funcionara harian falta mas muestras por cuadro (caro),
        // mas historia en el acumulador (arrastre) o un filtro espacial
        // guiado por profundidad y normal. Ninguna de las tres es barata, y
        // esta escena casi no tiene bordes de sombra duros que suavizar:
        // esta iluminada por ocho luces de colores que se superponen, asi
        // que las sombras ya son tenues y de bajo contraste.
        //
        // El muestreo estocastico por cuadro SI se quedo donde si rinde:
        // en el reflejo borroso de mas abajo, que cuesta un 1.5%.

        // Con uno que tape basta, asi que esto NO busca el impacto mas
        // cercano: pregunta objeto por objeto y se va con el primer si.
        // `occluded` ademas es una cuenta geometrica pelada, sin armar el
        // Intersect ni clonar el material, que es lo que hacia caro cada
        // rayo de sombra. Y `any` corta solo en cuanto uno responde true.
        //
        // Adentro de `occluded` ya se contempla que lo que brilla solo NO
        // hace sombra: sin eso, cada mota de hada pegada al techo le
        // estampaba un manchon negro encima.
        //
        // Ya no es si/no: es CUANTA luz llega. Lo opaco tapa del todo, pero
        // el agua y el cristal dejan pasar su peso de refraccion, asi que
        // el fondo de la piscina se ilumina a traves del agua y los
        // cristales proyectan sombras tenues. Se corta apenas alguien
        // tapa del todo.
        // EL CACHE DE SOMBRA: primero se le pregunta al que tapo la vez
        // anterior, antes de tocar el arbol.
        //
        // Un rayo de sombra bloqueado es barato (corta apenas encuentra al
        // primero que tapa) pero uno que ademas tiene que ENCONTRARLO paga
        // el recorrido hasta dar con el. Y las sombras son coherentes: dos
        // pixeles vecinos, para la misma luz, casi siempre los tapa el
        // mismo objeto, porque una sombra es una mancha y no un pixel
        // suelto. Guardando cual fue y probandolo primero, adentro de una
        // sombra el rayo se resuelve con UN test de geometria en vez de un
        // recorrido.
        //
        // No cambia ni un pixel, y por una razon precisa: si el que se
        // recuerda tapa del todo, el resultado ES cero pase lo que pase con
        // el resto del camino (las transmisiones se multiplican y ninguna
        // es negativa). Solo se puede cortar con el que tapa DEL TODO; si
        // apenas atenua, hay que recorrer igual, porque para la sombra
        // importa todo lo que haya en el camino.
        //
        // Es el truco de Haines y Greenberg de 1986, que sigue siendo la
        // mejor relacion entre lo que cuesta escribirlo y lo que ahorra.
        let mut shadow_factor = 1.0f32;
        let recordado = sombras.luces.get(numero_luz).copied().unwrap_or(usize::MAX);
        let resuelto = recordado != usize::MAX
            && objects[recordado].transmision(&shadow_origin, &light_dir, shadow_distance) <= 0.0;

        if !resuelto {
            arbol_sombras.recorrer(&shadow_origin, &light_dir, shadow_distance, |i, limite| {
                shadow_factor *= objects[i].transmision(&shadow_origin, &light_dir, shadow_distance);
                // El limite NO se acorta mientras algo de luz siga pasando:
                // para la sombra importa TODO lo que haya en el camino, no
                // lo mas cercano (dos vidrios atenuan dos veces). Pero en
                // cuanto algo tapa del todo, devolver cero corta el
                // recorrido: la luz ya no llega y lo que haya mas alla da
                // igual.
                if shadow_factor <= 0.0 {
                    // Y ESE es el que hay que recordar para el pixel
                    // siguiente.
                    if let Some(r) = sombras.luces.get_mut(numero_luz) {
                        *r = i;
                    }
                    0.0
                } else {
                    limite
                }
            });
            // Si no lo tapo nadie, se olvida al anterior: seguir
            // preguntando por un objeto que ya no esta en el camino es un
            // test de geometria regalado en cada pixel iluminado.
            if shadow_factor > 0.0 {
                if let Some(r) = sombras.luces.get_mut(numero_luz) {
                    *r = usize::MAX;
                }
            }
        } else {
            shadow_factor = 0.0;
        }

        // En sombra se apagan difuso y especular de ESTA luz, pero NO se
        // corta la funcion: las otras luces siguen aportando, y reflexion
        // y refraccion se calculan igual, porque un espejo o el agua se
        // siguen viendo aunque esten en sombra.
        if shadow_factor <= 0.0 {
            continue;
        }

        // La luz se apaga con la distancia: una luz alumbra donde ESTA, no
        // la cueva entera pareja.
        let shadow_factor = shadow_factor * light.atenuacion(shadow_distance);

        let reflect_dir = reflect(&-light_dir, &normal);

        // El difuso lleva el COLOR de la luz. Antes solo el especular lo
        // llevaba, y una luz rosa alumbraba la piedra en gris: la fuente
        // se leia cyan pareja por mas que las luces fueran rosas y
        // violetas. Ahora cada luz tine lo que toca, y donde se cruzan dos
        // de colores distintos el color se mezcla de verdad.
        let diffuse = tint_color(
            scale_color(
                base_color,
                intersect.material.albedo[0] * diffuse_intensity * light.intensity * shadow_factor,
            ),
            [
                light.color.r as f32 / 255.0,
                light.color.g as f32 / 255.0,
                light.color.b as f32 / 255.0,
            ],
        );

        let specular_intensity = dot(&view_dir, &reflect_dir)
            .max(0.0)
            .powf(intersect.material.specular);
        let specular = scale_color(
            light.color,
            intersect.material.albedo[1] * specular_intensity * light.intensity * shadow_factor,
        );

        lit = add_colors(lit, add_colors(diffuse, specular));
    }

    // --- REFLEXION ---
    // Se rebota el rayo sobre la normal y se vuelve a trazar.
    // El origen tambien se desplaza sobre la normal para no
    // volver a pegarle a la misma superficie.
    let mut reflection = Color::new(0, 0, 0, 255);
    let peso_reflexion = peso * peso_kr;
    if peso_reflexion > MIN_CONTRIBUCION {
        let mut reflection_dir = normalize(&reflect(direction, &normal));

        // REFLEJO BORROSO: la direccion se desvia un punto al azar de una
        // esfera del tamano de la rugosidad. Es la receta clasica (la
        // "fuzz" del metal de Ray Tracing in One Weekend) y aproxima un
        // lobulo especular ancho sin tener que muestrear una distribucion
        // de microfacetas: para rugosidades chicas, que son las de esta
        // escena, la diferencia no se ve.
        //
        // Un rayo por cuadro y el acumulador temporal hace el promedio, o
        // sea CERO rayos extra. Lo que si cambia es de donde viene el
        // ruido: con la camara quieta converge en unos cuatro cuadros, y
        // con la camara moviendose rapido el acumulador confia menos en la
        // historia y el reflejo se ve un poco granulado. Es el mismo
        // compromiso que hace cualquier motor en tiempo real.
        if intersect.material.rugosidad > 0.0 {
            let desvio = azar::en_esfera(semilla ^ 0x5bf0_3635) * intersect.material.rugosidad;
            reflection_dir = normalize(&(reflection_dir + desvio));
        }

        // El relieve puede inclinar la normal tanto que el rayo reflejado
        // apunte hacia ADENTRO de la superficie; se lo vuelve a sacar.
        let hacia_adentro = dot(&reflection_dir, &intersect.normal);
        if hacia_adentro < 0.0 {
            reflection_dir = normalize(&(reflection_dir - intersect.normal * (2.0 * hacia_adentro)));
        }
        let reflection_origin = intersect.point + intersect.normal * 1e-3;
        let (reflection_color, _) = cast_ray(
            &reflection_origin,
            &reflection_dir,
            objects,
            cielo,
            arbol,
            arbol_sombras,
            lights,
            depth + 1,
            max_depth,
            peso_reflexion,
            ambiente,
            fase_agua,
            // Cada rebote sortea distinto: con la misma semilla, el
            // reflejo de un reflejo se desviaria en la misma direccion y
            // el ruido saldria correlacionado entre niveles.
            azar::revolver(semilla ^ 0x9e37_79b9),
            sombras,
        );
        reflection = scale_color(reflection_color, peso_kr);
    }

    // --- REFRACCION ---
    // El rayo atraviesa el objeto: el origen se desplaza en direccion
    // OPUESTA a la normal porque el rayo ENTRA a la superficie.
    let mut refraction = Color::new(0, 0, 0, 255);
    let peso_refraccion = peso * peso_kt;
    if peso_refraccion > MIN_CONTRIBUCION {
        let refraction_dir = normalize(&refract(
            direction,
            &normal,
            intersect.material.refractive_index,
        ));
        // El desplazamiento sigue al rayo refractado: si atraviesa, sale
        // por debajo de la superficie; si hubo reflexion total interna,
        // vuelve por arriba.
        let lado = if dot(&refraction_dir, &intersect.normal) < 0.0 { -1.0 } else { 1.0 };
        let refraction_origin = intersect.point + intersect.normal * (1e-3 * lado);
        let (refraction_color, _) = cast_ray(
            &refraction_origin,
            &refraction_dir,
            objects,
            cielo,
            arbol,
            arbol_sombras,
            lights,
            depth + 1,
            max_depth,
            peso_refraccion,
            ambiente,
            fase_agua,
            azar::revolver(semilla ^ 0x1234_5679),
            sombras,
        );
        refraction = scale_color(refraction_color, peso_kt);
    }

    let shaded = add_colors(lit, add_colors(reflection, refraction));

    // --- EMISION ---
    // Luz propia del objeto: se suma al final, sin pasar por las luces
    // ni por las sombras. Por eso el cristal se ve encendido aunque
    // nada lo ilumine.
    let final_color = match intersect.material.emission_color {
        Some(emission) => add_colors(shaded, emission),
        None => shaded,
    };

    (final_color, zbuffer)
}


/// Traza la imagen completa y la deja lista. Se usa para la previa, que es
/// chica y entra en un cuadro.
/// Cuantas filas traza de un saque antes de dejar respirar al hilo.
///
/// No es una perilla de rendimiento: es LA razon de que la musica no se
/// corte. Ver el comentario de `render`.
const FILAS_POR_BANDA: usize = 75;

/// Traza la imagen completa y la deja lista para subir a la GPU.
///
/// La escena cambia en CADA cuadro (los keyframes le mueven la emision y las
/// luces), asi que esto corre entero en cada vuelta del loop. Por eso el
/// trazado esta paralelizado: sin rayon, a 800 x 600 no habria con que.
///
/// POR QUE VA DE A BANDAS, y no todo de una:
///
/// Un cuadro tarda ~350 ms, y todo ese rato el hilo principal esta adentro
/// de esta funcion. El reproductor de audio no vive en otro hilo: se rellena
/// llamando a `update_stream` desde el loop. Si el loop se ausenta 350 ms,
/// el buffer se vacia y el sonido se corta, por mas grande que se lo haga.
///
/// Cortando el trazado en bandas, el hilo vuelve a la superficie varias
/// veces por cuadro y ahi rellena el audio.
///
/// CUANTAS bandas es un compromiso medido. Cada una cuesta un despacho a
/// los hilos de rayon y una espera a que termine el ultimo, y ademas una
/// llamada a rellenar el audio, que decodifica mp3. Con bandas de 40 filas
/// eran ocho por cuadro y se median +1.8 ms solo de despachos, mas ocho
/// decodificaciones. Con 75 son cuatro, que a 34 ms por cuadro siguen
/// rellenando el audio cada ~8 ms: de sobra para que no se corte, y la
/// mitad de gasto.
///
/// El trazado hoy tarda 34 ms; cuando tardaba 350 (antes del BVH y de las
/// cajas acotantes) hacian falta muchas mas.
///
/// `entre_bandas` es lo que se corre en cada respiro. Va como parametro y no
/// clavado adentro para que esta funcion siga sin saber que existe el audio.
fn render(
    framebuffer: &mut Framebuffer,
    objects: &[Box<dyn RayIntersect + Send + Sync>],
    cielo: &Cielo,
    arbol: &Bvh,
    arbol_sombras: &Bvh,
    lights: &[Light],
    camera: &Camera,
    max_depth: u32,
    antialias: bool,
    // Donde cae la muestra DENTRO del pixel en este cuadro (ver el jitter
    // de `render_rows`). Cambia cuadro a cuadro: es lo que convierte la
    // acumulacion temporal en supermuestreo.
    jitter: (f32, f32),
    // La luz ambiente de este cuadro. Ver `cast_ray`.
    ambiente: [f32; 3],
    // La fase de las olas de este cuadro. Ver `cast_ray`.
    fase_agua: f32,
    // El numero de cuadro. Ver `render_rows`.
    cuadro: u32,
    mut entre_bandas: impl FnMut(),
) {
    let alto = framebuffer.height;
    let mut fila = 0;

    while fila < alto {
        let hasta = (fila + FILAS_POR_BANDA).min(alto);

        render_rows(
            framebuffer,
            objects,
            cielo,
            arbol,
            arbol_sombras,
            lights,
            camera,
            max_depth,
            antialias,
            jitter,
            ambiente,
            fase_agua,
            cuadro,
            fila,
            hasta,
        );

        entre_bandas();
        fila = hasta;
    }
}

/// LOS DOS ARBOLES. Se arman UNA vez y valen toda la corrida.
///
/// SE PROBO REARMARLOS EN CADA CUADRO Y NO SIRVE. Queda anotado para que no
/// se reintente a ciegas, porque el argumento para hacerlo es bueno: lo que
/// se mueve nace con una caja que cubre TODO su recorrido (la de cada grupo
/// de estelas es una esfera de ocho unidades alrededor del centro de la
/// escena), el arbol se queda con SU copia, y asi casi cualquier rayo que
/// entra a la cueva "toca" a las estelas y tiene que bajar a preguntarles
/// aunque no haya ninguna viva. Rearmar los dos arboles sale casi gratis
/// —treinta y tres objetos, unas dos mil cuentas de area— y deja las cajas
/// de AHORA.
///
/// Medido: 36.7, 38.9 y 37.1 ms contra 37.4, 37.7 y 38.1 sin rearmar, o sea
/// exactamente nada. El motivo es que ese trabajo YA estaba hecho en otro
/// lado: cada `GrupoAcotado` recalcula su propia caja en cada cuadro
/// (`recalcular_caja`), asi que el rayo que entra a la caja gorda del arbol
/// choca enseguida con la caja chica del grupo y se va. Lo que se ahorraba
/// rearmando era un test de caja por grupo, no los hijos.
fn arboles_de(objects: &[Box<dyn RayIntersect + Send + Sync>], occluders: &[usize]) -> (Bvh, Bvh) {
    let caja_de = |i: &usize| (*i, objects[*i].aabb());
    let todos: Vec<usize> = (0..objects.len()).collect();
    (
        Bvh::construir(&todos.iter().map(caja_de).collect::<Vec<_>>()),
        Bvh::construir(&occluders.iter().map(caja_de).collect::<Vec<_>>()),
    )
}

fn render_rows(
    framebuffer: &mut Framebuffer,
    objects: &[Box<dyn RayIntersect + Send + Sync>],
    cielo: &Cielo,
    arbol: &Bvh,
    arbol_sombras: &Bvh,
    lights: &[Light],
    camera: &Camera,
    max_depth: u32,
    // Con `true`, cuatro rayos por pixel en vez de uno. Cuadruplica el
    // costo, asi que va apagado por defecto y se prende con A.
    antialias: bool,
    // El corrimiento de la muestra dentro del pixel, en [0, 1). Con
    // (0.5, 0.5) el rayo sale por el centro, como siempre.
    jitter: (f32, f32),
    // La luz ambiente de este cuadro. Ver `cast_ray`.
    ambiente: [f32; 3],
    // La fase de las olas de este cuadro. Ver `cast_ray`.
    fase_agua: f32,
    // El numero de cuadro, que entra en la semilla de azar de cada pixel.
    // Tiene que CAMBIAR entre cuadros: es lo que hace que el acumulador
    // temporal promedie muestras distintas y los reflejos borrosos y las
    // sombras suaves converjan en vez de quedarse en una sola muestra.
    cuadro: u32,
    row_start: usize,
    row_end: usize,
) {
    let width = framebuffer.width as f32;
    let height = framebuffer.height as f32;

    let aspect_ratio = width / height;
    // 45 grados en vez de 60. Con el gran angular la espada salia diminuta
    // en el centro, y acercar la camara lo suficiente para verla dejaba al
    // templete fuera de cuadro. Cerrar el lente agranda el sujeto sin tener
    // que meterse adentro del edificio.
    let fov = std::f32::consts::PI / 4.0; // 45 grados
    let scale = (fov / 2.0).tan();

    // Los tres ejes de la camara. Se sacan una vez por render, no por pixel.
    let (right, up, forward) = camera.basis();

    let pixel_width = framebuffer.width;
    let pixel_height = framebuffer.height;

    // Cada pixel es independiente de los demas: nadie lee lo que
    // escribio otro. Por eso se pueden repartir entre todos los nucleos.
    // El framebuffer NO se toca aqui adentro (necesitaria &mut y eso no
    // se comparte entre hilos): se juntan los colores y se escriben despues.
    let row_end = row_end.min(pixel_height);
    if row_start >= row_end {
        return;
    }

    // Cada fila es independiente y se escribe donde va, sin juntarla
    // antes en ningun lado (ver `Framebuffer::filas_mut`).
    let (colores, profundidades) = framebuffer.filas_mut(row_start, row_end);

    colores
        .par_chunks_mut(pixel_width)
        .zip(profundidades.par_chunks_mut(pixel_width))
        .enumerate()
        .for_each(|(banda_y, (fila_color, fila_prof))| {
        let y = row_start + banda_y;
        // El cache de sombra de ESTA fila. Va aca y no afuera porque cada
        // fila corre en el nucleo que le toque: asi es de un solo hilo por
        // construccion, sin candados ni falso compartido. Y una fila es
        // justo la unidad en la que la coherencia sirve, porque los
        // pixeles vecinos de una fila caen casi siempre en la misma
        // sombra. Ver `Sombras`.
        let mut sombras = Sombras::vacio();
        for x in 0..pixel_width {

            // Donde cae cada muestra DENTRO del pixel.
            //
            // Sin antialiasing, una sola en el centro. Con antialiasing,
            // cuatro en una malla de 2x2, pero corridas (jittered): en vez
            // de los cuartos exactos, se desplazan un poco. Con la malla
            // perfecta los bordes en diagonal quedan con escalones de
            // cuatro niveles, que es una escalera mas fina pero escalera al
            // fin; el corrimiento rompe ese patron y el borde se lee suave.
            //
            // Y sin antialiasing la muestra unica NO va clavada en el
            // centro: va donde diga `jitter`, que cambia en cada cuadro.
            // Sola no serviria de nada (un rayo por pixel es un rayo por
            // pixel, salga por donde salga), pero el acumulador temporal
            // promedia los cuadros, asi que lo que llega a la pantalla es
            // el promedio de las ultimas muestras: supermuestreo repartido
            // en el tiempo, a costo de un rayo.
            let centro: [(f32, f32); 1] = [jitter];
            const MALLA: [(f32, f32); 4] = [
                (0.30, 0.20),
                (0.75, 0.35),
                (0.20, 0.70),
                (0.65, 0.85),
            ];

            let muestras: &[(f32, f32)] = if antialias { &MALLA } else { &centro };

            let mut suma = (0.0f32, 0.0f32, 0.0f32);
            // La profundidad NO se promedia: se guarda la mas cercana. El
            // depth buffer alimenta la niebla y el desenfoque, y promediar
            // el borde de un objeto con el fondo infinito de atras dejaria
            // un halo de profundidad falsa alrededor de cada silueta.
            let mut mas_cerca = f32::MAX;

            for (jx, jy) in muestras {
                // 1. Pixel -> espacio normalizado [-1, 1].
                let mut screen_x = (2.0 * (x as f32 + jx)) / width - 1.0;
                let mut screen_y = 1.0 - (2.0 * (y as f32 + jy)) / height;
                //                 ^ Y invertida: fila 0 esta arriba en el
                //                   framebuffer, pero arriba es +Y para la camara.

                // 2. Aspect ratio. Sin esto la esfera sale como elipse.
                screen_x *= aspect_ratio;

                // 3. Campo de vision.
                screen_x *= scale;
                screen_y *= scale;

                // 4. Del espacio de la camara al mundo: en vez de mirar
                //    siempre a -Z, el rayo se arma con la base de la camara.
                let direction = normalize(&(right * screen_x + up * screen_y + forward));

                let (color, distancia) =
                    cast_ray(
                        &camera.position,
                        &direction,
                        objects,
                        cielo,
                        arbol,
                        arbol_sombras,
                        lights,
                        0,
                        max_depth,
                        1.0,
                        ambiente,
                        fase_agua,
                        azar::semilla(x, y, cuadro),
                        &mut sombras,
                    );

                suma.0 += color.r as f32;
                suma.1 += color.g as f32;
                suma.2 += color.b as f32;
                mas_cerca = mas_cerca.min(distancia);
            }

            let n = muestras.len() as f32;
            fila_color[x] = Color::new(
                (suma.0 / n).clamp(0.0, 255.0) as u8,
                (suma.1 / n).clamp(0.0, 255.0) as u8,
                (suma.2 / n).clamp(0.0, 255.0) as u8,
                255,
            );
            fila_prof[x] = mas_cerca;
        }
    });
}

// ============================================================
//  POST-PROCESADO EN LA GPU
// ============================================================

/// Donde viven los tres shaders. La ruta es relativa al directorio desde
/// donde se corre el programa, que con `cargo run` es la raiz del proyecto.
const SHADERS: &str = "resources/shaders/glsl330";

/// Resolucion a la que se calcula el bloom: la mitad de la ventana.
///
/// No es un recorte de calidad, es lo que hace que el efecto sea barato Y
/// ancho a la vez. El halo es por definicion algo sin detalle, asi que
/// nadie ve la diferencia entre calcularlo a 800 o a 400; en cambio, a
/// mitad de resolucion cada muestra del desenfoque cubre dos pixeles de
/// pantalla, o sea que el mismo numero de muestras alcanza el doble de
/// radio. Y de paso son cuatro veces menos pixeles que sombrear.
const BLOOM_W: u32 = (WIDTH / 2) as u32;
const BLOOM_H: u32 = (HEIGHT / 2) as u32;

// El UMBRAL del bloom ya no vive aca: lo decide la cancion cuadro a cuadro
// porque depende de la hora del dia (ver `SceneParams::bloom_threshold` en
// `sync.rs`). Lo que pasa el umbral sigue siendo lo mismo de siempre —el
// agua, las hadas, las molduras de oro, la Triforce y los brillos del
// marmol— y sigue siendo ademas la fuente de los god rays.

/// Cuantos niveles tiene la cadena de mips del bloom, contando el primero
/// (que ya esta a mitad de la ventana).
///
/// Seis: 400x300, 200x150, 100x75, 50x37, 25x18 y 12x9. El ultimo es el
/// que da la falda ancha del halo —doce pixeles de ancho estirados a la
/// pantalla entera hacen que un punto de luz tina medio cuadro— y por
/// debajo de eso no queda imagen, solo un color plano. Mas niveles no
/// agregan nada y menos dejan el halo corto.
const BLOOM_NIVELES: usize = 6;

/// El radio del filtro de carpa al subir la cadena, en coordenadas de
/// textura.
///
/// Es chico a proposito: la mayor parte del ancho del halo la da la CADENA
/// (cada nivel es el doble de grande que el anterior), no el filtro. Un
/// radio grande aca no ensancha, emborrona: separa las nueve muestras de la
/// carpa hasta que se ven como nueve copias, igual que le pasaba al
/// gaussiano viejo.
const BLOOM_RADIO_CARPA: f32 = 0.0045;

/// A que "dispersion" del filtro de subida corresponde un punto del radio
/// que pide la tabla de la cancion.
///
/// La dispersion es cuanto pesa cada nivel de la cadena sobre el anterior
/// (ver `bloom_up.fs`): con 0.35 mandan los niveles grandes y el halo queda
/// apretado contra el objeto; con 0.75 mandan los chicos y se derrama por
/// el cuadro. La tabla mueve `bloom_radius` entre 5 (intro) y 14 (coro),
/// asi que esta escala y este piso lo llevan de 0.42 a 0.73.
/// EL RANGO ES ANGOSTO A PROPOSITO, y esto arregla un defecto que se
/// reportaba como "la camara se reenfoca a veces".
///
/// La dispersion decide cuanto pesan los niveles chicos de la cadena, o
/// sea CUANTO SE DERRAMA el halo. Antes iba de 0.435 a 0.732 siguiendo al
/// bajo, y medido sobre la cancion podia recorrer el 75% de ese rango en
/// tres segundos. Se rendearon los dos extremos con todo lo demas igual y
/// la diferencia es inconfundible: con la dispersion baja la Triforce
/// tiene un halo apretado y el cuadro se lee nitido, y con la alta el
/// resplandor se reparte por todas partes y el cuadro entero parece
/// desenfocado. Eso, yendo y viniendo cada pocos segundos, es exactamente
/// lo que el ojo interpreta como un foco que se corrige solo.
///
/// (Antes de llegar aca se descartaron dos sospechosos midiendo: el peso
/// del acumulador temporal, que a dieciocho cuadros por segundo solo va de
/// 0.253 a 0.298 y no alcanza para nada; y la profundidad de campo, que
/// con la banda nitida en 0.20 deja TODA la fuente en desenfoque cero en
/// todo momento.)
///
/// Con 0.44 a 0.58 el halo sigue respirando con el bajo pero su ANCHO
/// queda casi quieto. La separacion es la que corresponde: la intensidad
/// del bloom lleva la musica, y el ancho lleva el look. El ancho, si se
/// mueve mucho, no se lee como musica: se lee como un defecto de camara.
const BLOOM_DISPERSION_BASE: f32 = 0.362;
const BLOOM_DISPERSION_ESCALA: f32 = 0.0156;
const BLOOM_DISPERSION_MAXIMA: f32 = 0.60;

/// Entre que profundidades se abre el caleidoscopio, en la escala del canal
/// alpha (0 = pegado a la camara, 1 = `PROFUNDIDAD_MAXIMA`).
///
/// NO SON NUMEROS ELEGIDOS A OJO: salen del histograma de profundidad de un
/// cuadro real de esta escena, que es marcadamente de dos jorobas. El 40% de
/// los pixeles cae entre 3 y 15 unidades (el piso cercano, la esfera de
/// cristal, los postes) y otro 53% entre 25 y 31 (la pared del fondo y el
/// techo). Entre 15 y 25 unidades hay un valle donde vive apenas el 7%, y
/// ahi es donde conviene cortar: la transicion pasa por donde casi no hay
/// nada, asi que ningun objeto queda partido al medio por la mascara.
///
/// 0.35 y 0.50 de alpha son 17.5 y 25 unidades, las dos paredes del valle.
const KAL_PROF_CERCA: f32 = 0.35;
const KAL_PROF_LEJOS: f32 = 0.50;

/// Los tres shaders y las posiciones de sus uniforms.
///
/// Las locations se buscan UNA VEZ al arrancar y no por cuadro:
/// `GetShaderLocation` hace una consulta al driver por nombre, y son diez
/// nombres por cuadro que devuelven siempre lo mismo.
struct PostGpu {
    threshold: Shader,
    baja: Shader,
    sube: Shader,
    composite: Shader,
    godrays: Shader,
    kaleidoscope: Shader,
    effects: Shader,

    // `threshold` no esta aca: vale lo mismo toda la corrida, asi que se
    // manda una sola vez en `nuevo` y su location no vuelve a hacer falta.
    // La resolucion de la bajada SI cambia en cada nivel de la cadena, y
    // la dispersion de la subida la mueve la cancion.
    // El umbral SE MUEVE con la hora del dia (ver `SceneParams::
    // bloom_threshold`), asi que su location tiene que quedar guardada.
    loc_threshold: i32,
    loc_baja_res: i32,
    loc_sube_scatter: i32,

    loc_bloom_tex: i32,
    loc_bloom_str: i32,
    loc_fog_density: i32,
    loc_fog_color: i32,
    loc_tint: i32,
    loc_resolution: i32,

    // Los god rays: solo la posicion de la luz cambia por cuadro (la camara
    // se mueve); densidad, peso, decaimiento, exposicion y muestras valen
    // lo mismo toda la corrida y se mandan una vez en `nuevo`. El halo del
    // bloom se ata en cada pasada, como en el composite.
    loc_luz_pantalla: i32,
    loc_gr_bloom: i32,
    // La exposicion de los rayos ya no es constante: la mueve el golpe de
    // la cancion en cada cuadro. Ver `GODRAYS_PULSO`.
    loc_gr_exposure: i32,

    // `center` del caleidoscopio tampoco esta aca, por lo mismo: el eje del
    // pliegue no se mueve, se manda una vez en `nuevo`.
    loc_kal_segments: i32,
    loc_kal_rotation: i32,
    loc_kal_mix: i32,
    loc_kal_depth: i32,

    loc_chromatic: i32,
    loc_grain: i32,
    loc_time: i32,
    loc_focus: i32,
    loc_dof: i32,
    loc_fx_depth: i32,
    loc_streak: i32,
    loc_letterbox: i32,
    loc_fx_bloom: i32,

    /// LA CADENA DE MIPS DEL BLOOM: `mips[0]` esta a mitad de la ventana y
    /// cada uno siguiente es la mitad del anterior.
    ///
    /// La cadena se recorre dos veces por cuadro: bajando (cada nivel se
    /// llena reduciendo el de arriba) y subiendo (cada nivel se mezcla
    /// sobre el de arriba). Al terminar, el halo completo —la suma de las
    /// seis escalas— esta en `mips[0]`, que es el que leen la composicion,
    /// los god rays y las estelas.
    ///
    /// Nunca hace falta un buffer de ida y vuelta como en el desenfoque
    /// separable de antes, porque cada pasada lee un nivel y escribe en
    /// OTRO: no hay ninguna que lea y escriba la misma textura.
    mips: Vec<RenderTexture2D>,

    /// Los tres cuadros intermedios de la ventana: `escena` es la salida
    /// del composite y la entrada de los god rays; `rayos` la salida de los
    /// god rays y la entrada del caleidoscopio; `plegada` la salida del
    /// caleidoscopio y la entrada de los efectos.
    ///
    /// Hacen falta porque las tres pasadas leen pixeles LEJOS del que estan
    /// calculando: los god rays caminan hacia la luz, el caleidoscopio lee
    /// el del sector espejado y los efectos leen tres puntos distintos para
    /// separar los canales. Para eso la imagen tiene que estar entera en
    /// algun lado antes de empezar. Solo la ultima pasada puede dibujar
    /// directo a pantalla, porque es la unica que no vuelve a leer lo que
    /// produce.
    ///
    /// Van a 800 x 600 y no a media resolucion como los del bloom: estos son
    /// el cuadro que se ve, no un halo, y bajarlos tiraria la nitidez que el
    /// estirado bilineal acaba de dar.
    escena: RenderTexture2D,
    rayos: RenderTexture2D,
    plegada: RenderTexture2D,
}

/// Los parametros fijos de los god rays.
const GODRAYS_DENSITY: f32 = 0.68;
const GODRAYS_WEIGHT: f32 = 0.2;
const GODRAYS_DECAY: f32 = 0.95;
// BAJADA de 0.5 a 0.24 junto con el cambio de bloom. Los god rays caminan
// hacia la luz acumulando el halo, asi que su brillo final es proporcional
// a cuanta energia tenga el halo; la cadena de mips entrega bastante mas
// que el gaussiano de dos pasadas que habia antes, y con la exposicion
// vieja los rayos dejaron de ser rayos y pasaron a ser un velo blanco
// sobre la mitad de arriba del cuadro. Apagandolos del todo, a modo de
// prueba, el climax se leia de inmediato; con este valor se leen los rayos
// Y se lee la fuente.
/// EXPOSICION DE BASE de los rayos, la que tienen entre golpe y golpe.
///
/// El valor sube con `GODRAYS_PULSO` en cada tiempo. Ver `god_rays`.
const GODRAYS_EXPOSURE: f32 = 0.25;

/// CUANTO SE ENCIENDEN LOS HACES EN EL GOLPE.
///
/// AQUI ES DONDE VIVE EL PULSO DE LA CANCION, y es un cambio de criterio.
/// Antes el golpe se aplicaba al cuadro ENTERO desde el composite: una
/// subida de contraste y saturacion y una caida de exposicion entre
/// tiempos. Funcionaba —medido, treinta por ciento de luminancia y veinte
/// de contraste entre estar en el uno y estar entre golpes— pero se sentia
/// como un visualizador de musica: la pantalla pegaba, no la escena.
///
/// Los haces de luz hacen el mismo trabajo y son ATMOSFERA en vez de
/// efecto. Lo que late no es la imagen sino la luz que baja por el hueco
/// del techo, que es una cosa que esta pasando ADENTRO de la cueva. El ojo
/// lee lo mismo (algo se enciende al ritmo) y el cuadro no se sacude.
///
/// El rango va de 0.17 a 0.55. Ese 0.55 es mas alto que el 0.5 que en su
/// momento hubo que bajar por reventar el cuadro, y no es contradiccion:
/// aquello era un valor SOSTENIDO y esto es un pico de 120 milisegundos.
/// Lo que como promedio permanente era un velo blanco, como transitorio es
/// un destello.
const GODRAYS_PULSO: f32 = 0.45;
const GODRAYS_SAMPLES: i32 = 60;

impl PostGpu {
    fn nuevo(rl: &mut RaylibHandle, thread: &RaylibThread) -> PostGpu {
        // Vertex shader `None`: raylib pone el suyo, que es exactamente lo
        // que hace falta (pasar posicion y coordenada de textura de un
        // rectangulo). Todo el trabajo esta en el de fragmentos.
        let threshold =
            rl.load_shader(thread, None, Some(&format!("{SHADERS}/bloom_threshold.fs")));
        let baja = rl.load_shader(thread, None, Some(&format!("{SHADERS}/bloom_down.fs")));
        let mut sube = rl.load_shader(thread, None, Some(&format!("{SHADERS}/bloom_up.fs")));
        let composite = rl.load_shader(thread, None, Some(&format!("{SHADERS}/composite.fs")));
        let mut godrays = rl.load_shader(thread, None, Some(&format!("{SHADERS}/godrays.fs")));
        let mut kaleidoscope =
            rl.load_shader(thread, None, Some(&format!("{SHADERS}/kaleidoscope.fs")));
        let effects = rl.load_shader(thread, None, Some(&format!("{SHADERS}/effects.fs")));

        for (shader, nombre) in [
            (&threshold, "bloom_threshold.fs"),
            (&baja, "bloom_down.fs"),
            (&sube, "bloom_up.fs"),
            (&composite, "composite.fs"),
            (&godrays, "godrays.fs"),
            (&kaleidoscope, "kaleidoscope.fs"),
            (&effects, "effects.fs"),
        ] {
            // Si un .fs no compila, raylib deja el shader por defecto y la
            // escena se dibuja SIN post-procesado, que es peor que un error:
            // se ve casi bien y uno busca el problema en la escena.
            assert!(
                shader.is_shader_valid(),
                "no se pudo compilar {SHADERS}/{nombre} (mirar el log de raylib)"
            );
        }

        let loc_threshold = threshold.get_shader_location("threshold");
        let loc_baja_res = baja.get_shader_location("srcResolution");
        let loc_sube_radio = sube.get_shader_location("filterRadius");
        let loc_sube_scatter = sube.get_shader_location("scatter");

        let loc_bloom_tex = composite.get_shader_location("bloomTex");
        let loc_bloom_str = composite.get_shader_location("bloomStrength");
        let loc_fog_density = composite.get_shader_location("fogDensity");
        let loc_fog_color = composite.get_shader_location("fogColor");
        let loc_tint = composite.get_shader_location("colorTint");
        let loc_resolution = composite.get_shader_location("resolution");

        let loc_luz_pantalla = godrays.get_shader_location("lightScreenPos");
        let loc_gr_bloom = godrays.get_shader_location("bloomTex");
        let loc_gr_density = godrays.get_shader_location("density");
        let loc_gr_weight = godrays.get_shader_location("weight");
        let loc_gr_decay = godrays.get_shader_location("decay");
        let loc_gr_exposure = godrays.get_shader_location("exposure");
        let loc_gr_samples = godrays.get_shader_location("numSamples");

        let loc_kal_segments = kaleidoscope.get_shader_location("segments");
        let loc_kal_rotation = kaleidoscope.get_shader_location("rotation");
        let loc_kal_mix = kaleidoscope.get_shader_location("kMix");
        let loc_kal_center = kaleidoscope.get_shader_location("center");
        let loc_kal_depth = kaleidoscope.get_shader_location("depthTex");
        let loc_kal_rango = kaleidoscope.get_shader_location("depthRange");

        let loc_chromatic = effects.get_shader_location("chromatic");
        let loc_grain = effects.get_shader_location("grainAmount");
        let loc_time = effects.get_shader_location("time");
        let loc_focus = effects.get_shader_location("focusDepth");
        let loc_dof = effects.get_shader_location("dofAmount");
        let loc_fx_depth = effects.get_shader_location("depthTex");
        let loc_streak = effects.get_shader_location("streak");
        let loc_letterbox = effects.get_shader_location("letterbox");
        let loc_fx_bloom = effects.get_shader_location("bloomTex");

        // Constantes por toda la corrida: se mandan una sola vez.
        sube.set_shader_value(loc_sube_radio, BLOOM_RADIO_CARPA);
        // El eje del pliegue es el centro de la pantalla y no se mueve. Esta
        // como uniform y no clavado en el shader por si alguna vez se quiere
        // desplazar con la camara.
        kaleidoscope.set_shader_value(loc_kal_center, [0.5f32, 0.5f32]);
        kaleidoscope.set_shader_value(loc_kal_rango, [KAL_PROF_CERCA, KAL_PROF_LEJOS]);
        godrays.set_shader_value(loc_gr_density, GODRAYS_DENSITY);
        godrays.set_shader_value(loc_gr_weight, GODRAYS_WEIGHT);
        godrays.set_shader_value(loc_gr_decay, GODRAYS_DECAY);
        godrays.set_shader_value(loc_gr_samples, GODRAYS_SAMPLES);

        // La cadena, cada nivel la mitad del anterior. El `max(1)` es por
        // si alguien sube BLOOM_NIVELES lo suficiente como para que un
        // nivel quede en cero pixeles, que raylib no acepta.
        let mips: Vec<RenderTexture2D> = (0..BLOOM_NIVELES)
            .map(|i| {
                let w = (BLOOM_W >> i).max(1);
                let h = (BLOOM_H >> i).max(1);
                rl.load_render_texture(thread, w, h)
                    .unwrap_or_else(|_| panic!("no se pudo crear el nivel {i} del bloom"))
            })
            .collect();

        let escena = rl
            .load_render_texture(thread, WIDTH as u32, HEIGHT as u32)
            .expect("no se pudo crear el buffer de la escena");
        let rayos = rl
            .load_render_texture(thread, WIDTH as u32, HEIGHT as u32)
            .expect("no se pudo crear el buffer de los god rays");
        let plegada = rl
            .load_render_texture(thread, WIDTH as u32, HEIGHT as u32)
            .expect("no se pudo crear el buffer del caleidoscopio");

        for rt in mips.iter().chain([&escena, &rayos, &plegada]) {
            // Bilineal: el halo se lee estirado al doble en la composicion,
            // y con el filtro de vecino mas cercano se verian los bloques de
            // 2x2 pixeles del buffer de bloom dentro del resplandor.
            rt.texture()
                .set_texture_filter(thread, TextureFilter::TEXTURE_FILTER_BILINEAR);
            // Y sin repetir: el desenfoque pide muestras mas alla del borde,
            // y con el modo por defecto esas muestras vienen del lado
            // OPUESTO de la imagen. Un laser pegado al borde derecho
            // terminaria dejando un resplandor contra el izquierdo.
            rt.texture()
                .set_texture_wrap(thread, TextureWrap::TEXTURE_WRAP_CLAMP);
        }

        PostGpu {
            threshold,
            baja,
            sube,
            composite,
            godrays,
            kaleidoscope,
            effects,
            loc_threshold,
            loc_baja_res,
            loc_sube_scatter,
            loc_bloom_tex,
            loc_bloom_str,
            loc_fog_density,
            loc_fog_color,
            loc_tint,
            loc_resolution,
            loc_luz_pantalla,
            loc_gr_bloom,
            loc_gr_exposure,
            loc_kal_segments,
            loc_kal_rotation,
            loc_kal_mix,
            loc_kal_depth,
            loc_chromatic,
            loc_grain,
            loc_time,
            loc_focus,
            loc_dof,
            loc_fx_depth,
            loc_streak,
            loc_letterbox,
            loc_fx_bloom,
            mips,
            escena,
            rayos,
            plegada,
        }
    }

    /// Del cuadro trazado al halo, que queda guardado en `self.mips[0]`.
    ///
    /// Tres etapas: el umbral llena el nivel 0, la BAJADA reduce por la
    /// cadena hasta el nivel mas chico, y la SUBIDA vuelve mezclando cada
    /// nivel sobre el de arriba. Ver `bloom_down.fs` y `bloom_up.fs`.
    ///
    /// Va ANTES de `begin_drawing` a proposito. `BeginTextureMode` se basta
    /// solo (pone su propio viewport y su propia proyeccion) y
    /// `EndTextureMode` restaura los de la pantalla, asi que meterlo dentro
    /// del dibujado funcionaria igual; pero dejarlo afuera separa lo que se
    /// pinta en buffers de lo que se pinta en la ventana.
    fn armar_bloom(
        &mut self,
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
        tex_rt: &Texture2D,
        radio: f32,
        umbral: f32,
    ) {
        // El umbral se manda por cuadro y no una sola vez al arrancar: lo
        // mueve la hora del dia. Ver `SceneParams::bloom_threshold`.
        self.threshold.set_shader_value(self.loc_threshold, umbral);

        // Cuanto pesa cada nivel de la cadena sobre el de arriba: es la
        // perilla de ancho del halo que mueve la cancion.
        let dispersion = (BLOOM_DISPERSION_BASE + radio * BLOOM_DISPERSION_ESCALA)
            .clamp(0.2, BLOOM_DISPERSION_MAXIMA);
        self.sube.set_shader_value(self.loc_sube_scatter, dispersion);

        // --- UMBRAL: quedarse con lo que quema, ya a mitad de resolucion.
        //
        // El alto del origen va POSITIVO: `tex_rt` es una textura normal,
        // subida desde memoria, y sus filas ya estan en el orden en que
        // raylib dibuja.
        {
            let PostGpu { threshold, mips, .. } = self;
            let origen = Rectangle {
                x: 0.0,
                y: 0.0,
                width: RENDER_W as f32,
                height: RENDER_H as f32,
            };
            let destino = Rectangle {
                x: 0.0,
                y: 0.0,
                width: BLOOM_W as f32,
                height: BLOOM_H as f32,
            };
            let mut tb = rl.begin_texture_mode(thread, &mut mips[0]);
            let mut sm = tb.begin_shader_mode(threshold);
            sm.draw_texture_pro(tex_rt, origen, destino, Vector2::zero(), 0.0, Color::WHITE);
        }

        // De aca en adelante siempre se lee de un RenderTexture, y esos van
        // con el alto del origen NEGATIVO: OpenGL numera las filas de un
        // framebuffer de abajo hacia arriba mientras que raylib dibuja de
        // arriba hacia abajo, asi que lo que se pinto arriba quedo guardado
        // abajo. El signo menos da vuelta el muestreo y lo compensa.
        //
        // Lo que hay que cuidar es la PARIDAD, porque la subida mezcla
        // sobre contenido que ya estaba: si el nivel que llega y el que ya
        // esta guardado no coinciden en orientacion, el halo se mezcla
        // consigo mismo del reves. Sale bien solo: bajando, el nivel `k`
        // queda con `k` vueltas (el 0 con ninguna, que lo escribio el
        // umbral sin invertir); subiendo, lo que entra al nivel `k` trae
        // una vuelta mas que el nivel `k+1`, o sea `k+2`, que tiene la
        // misma paridad que las `k` que ya tenia. Y al final `mips[0]`
        // queda con paridad par, igual que lo dejo el umbral, asi que la
        // composicion lo sigue leyendo con el `1.0 - y` de siempre.
        let tam = |i: usize| ((BLOOM_W >> i).max(1) as f32, (BLOOM_H >> i).max(1) as f32);

        // Los campos por separado: las dos pasadas parten `mips` en dos con
        // `split_at_mut`, y con `self` entero el compilador ve un prestamo
        // mutable de todo y no deja usar los shaders al mismo tiempo.
        let PostGpu { baja, sube, mips, loc_baja_res, .. } = self;

        // --- BAJADA: del nivel 0 al mas chico, reduciendo a la mitad.
        for i in 1..BLOOM_NIVELES {
            let (sw, sh) = tam(i - 1);
            let (dw, dh) = tam(i);

            // La resolucion del nivel que se LEE: el filtro de trece
            // muestras necesita saber cuanto mide un texel del origen.
            baja.set_shader_value(*loc_baja_res, [sw, sh]);

            // `split_at_mut` es lo que deja tener a la vez la referencia de
            // solo lectura al nivel de arriba y la mutable al de abajo. Sin
            // esto habria que indexar `self.mips` dos veces y el compilador
            // ve dos prestamos del mismo vector.
            let (arriba, abajo) = mips.split_at_mut(i);
            let fuente = arriba[i - 1].texture();

            let mut tb = rl.begin_texture_mode(thread, &mut abajo[0]);
            let mut sm = tb.begin_shader_mode(&mut *baja);
            sm.draw_texture_pro(
                fuente,
                Rectangle { x: 0.0, y: 0.0, width: sw, height: -sh },
                Rectangle { x: 0.0, y: 0.0, width: dw, height: dh },
                Vector2::zero(),
                0.0,
                Color::WHITE,
            );
        }

        // --- SUBIDA: del mas chico al nivel 0, mezclando sobre el de
        // arriba con el filtro de carpa.
        //
        // La mezcla la hace el mezclador de salida de la GPU en modo ALFA,
        // con el alfa que escribe `bloom_up.fs`: `destino*(1-a) +
        // origen*a`. Ver el comentario largo de ese shader sobre por que se
        // interpola en vez de sumar como en el articulo original.
        for i in (1..BLOOM_NIVELES).rev() {
            let (sw, sh) = tam(i);
            let (dw, dh) = tam(i - 1);

            let (arriba, abajo) = mips.split_at_mut(i);
            let fuente = abajo[0].texture();

            let mut tb = rl.begin_texture_mode(thread, &mut arriba[i - 1]);
            let mut bm = tb.begin_blend_mode(BlendMode::BLEND_ALPHA);
            let mut sm = bm.begin_shader_mode(&mut *sube);
            sm.draw_texture_pro(
                fuente,
                Rectangle { x: 0.0, y: 0.0, width: sw, height: -sh },
                Rectangle { x: 0.0, y: 0.0, width: dw, height: dh },
                Vector2::zero(),
                0.0,
                Color::WHITE,
            );
        }
    }

    /// Pasada de composicion: junta el cuadro trazado con el halo y aplica
    /// niebla, tinte, vinieta y compresion de rango.
    ///
    /// NO dibuja en pantalla: escribe en `escena`. El caleidoscopio que
    /// viene despues necesita leer pixeles lejos del que esta calculando
    /// (por definicion: lee el del sector espejado), y eso obliga a tener la
    /// imagen entera terminada en algun lado antes de empezar a plegarla.
    fn componer(
        &mut self,
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
        tex_rt: &Texture2D,
        p: &SceneParams,
    ) {
        // Igual que en el bloom: los campos por separado, porque hay que
        // leer la textura del halo mientras se escribe en la de la escena.
        let PostGpu {
            composite,
            mips,
            escena,
            loc_bloom_tex,
            loc_bloom_str,
            loc_fog_density,
            loc_fog_color,
            loc_tint,
            loc_resolution,
            ..
        } = self;

        composite.set_shader_value(*loc_bloom_str, p.bloom_strength);
        composite.set_shader_value(*loc_fog_density, p.fog_density);
        composite.set_shader_value(
            *loc_fog_color,
            [p.fog_color.x, p.fog_color.y, p.fog_color.z],
        );
        composite.set_shader_value(
            *loc_tint,
            [p.color_shift.x, p.color_shift.y, p.color_shift.z],
        );
        composite.set_shader_value(*loc_resolution, [WIDTH as f32, HEIGHT as f32]);

        // El origen es el buffer trazado (mas chico que la ventana); el
        // destino es la ventana entera. Ese estirado sale gratis: es la
        // interpolacion bilineal de la GPU, la misma que hacia
        // `draw_texture_ex`, solo que ahora pasa por el shader.
        //
        // Alto POSITIVO: `tex_rt` es una textura normal, subida desde
        // memoria, y sus filas ya estan en el orden en que raylib dibuja.
        let origen = Rectangle {
            x: 0.0,
            y: 0.0,
            width: RENDER_W as f32,
            height: RENDER_H as f32,
        };
        let destino = Rectangle {
            x: 0.0,
            y: 0.0,
            width: WIDTH as f32,
            height: HEIGHT as f32,
        };

        // El segundo sampler. `texture0` lo ata raylib solo con la textura
        // que se esta dibujando; el halo hay que atarlo a mano, Y HAY QUE
        // HACERLO DESPUES DE `begin_shader_mode`, no antes.
        //
        // El motivo esta enterrado en rlgl. `SetShaderValueTexture` no ata
        // la textura: le busca una unidad libre y anota el id en una tabla
        // (`activeTextureId`) que se consume recien cuando se vacia el lote
        // de dibujo. Y `BeginShaderMode` VACIA ESE LOTE al cambiar de
        // programa, y de paso pone esa tabla en cero. O sea que la anotacion
        // hecha antes se pierde entera, sin ningun error: el shader queda
        // leyendo la textura nula y el bloom sale negro, que se parece
        // demasiado a "todavia no hay nada que brille".
        //
        // Invirtiendo el orden, la anotacion se hace con el programa ya
        // puesto y sobrevive hasta el dibujo. Va por FFI porque el shader ya
        // esta prestado y no se lo puede volver a pedir: se copia el handle
        // (que son dos numeros) antes de entrar. `SetShaderValue*` solo hace
        // `glUseProgram` + `glUniform`, no toca el lote, asi que hacerlo con
        // la copia es exactamente lo mismo que hacerlo con el original.
        let handle = *composite.as_ref();
        let halo = *mips[0].texture().as_ref();
        let loc_halo = *loc_bloom_tex;

        let mut tb = rl.begin_texture_mode(thread, escena);
        let mut sm = tb.begin_shader_mode(composite);
        unsafe { raylib::ffi::SetShaderValueTexture(handle, loc_halo, halo) };
        sm.draw_texture_pro(tex_rt, origen, destino, Vector2::zero(), 0.0, Color::WHITE);
    }

    /// Los rayos de luz que bajan por el hueco del techo.
    ///
    /// Lee `escena` (ya con bloom, niebla y ACES) como base y el halo del
    /// bloom (`a`) como fuente de los rayos, y escribe en `rayos`.
    /// `luz_pantalla` es donde cae la luz cenital en la pantalla, en UV, ya
    /// proyectada con la camara del cuadro.
    fn god_rays(
        &mut self,
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
        luz_pantalla: [f32; 2],
        pulso: f32,
    ) {
        // Los haces se encienden con el golpe. Ver `GODRAYS_PULSO`.
        self.godrays.set_shader_value(
            self.loc_gr_exposure,
            GODRAYS_EXPOSURE + GODRAYS_PULSO * pulso,
        );

        let PostGpu {
            godrays,
            mips,
            escena,
            rayos,
            loc_luz_pantalla,
            loc_gr_bloom,
            ..
        } = self;

        godrays.set_shader_value(*loc_luz_pantalla, luz_pantalla);

        // El segundo sampler se ata DESPUES de `begin_shader_mode`, por lo
        // mismo que en `componer` (ver ahi el porque).
        let handle = *godrays.as_ref();
        let halo = *mips[0].texture().as_ref();
        let loc_halo = *loc_gr_bloom;

        // Alto NEGATIVO: `escena` es un RenderTexture (ver `caleidoscopio`).
        let origen = Rectangle {
            x: 0.0,
            y: 0.0,
            width: WIDTH as f32,
            height: -(HEIGHT as f32),
        };
        let destino = Rectangle {
            x: 0.0,
            y: 0.0,
            width: WIDTH as f32,
            height: HEIGHT as f32,
        };

        let fuente = escena.texture();
        let mut tb = rl.begin_texture_mode(thread, rayos);
        let mut sm = tb.begin_shader_mode(godrays);
        unsafe { raylib::ffi::SetShaderValueTexture(handle, loc_halo, halo) };
        sm.draw_texture_pro(fuente, origen, destino, Vector2::zero(), 0.0, Color::WHITE);
    }

    /// Pliega la escena sobre si misma en sectores espejados.
    ///
    /// Tampoco dibuja en pantalla: escribe en `plegada`, porque despues
    /// vienen los efectos y esos leen tres puntos por pixel.
    fn caleidoscopio(
        &mut self,
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
        // El cuadro crudo del trazador. No se dibuja: se lee su alpha, que
        // es la profundidad, para saber que parte de la imagen es fondo.
        tex_rt: &Texture2D,
        p: &SceneParams,
    ) {
        let PostGpu {
            kaleidoscope,
            rayos,
            plegada,
            loc_kal_segments,
            loc_kal_rotation,
            loc_kal_mix,
            loc_kal_depth,
            ..
        } = self;

        kaleidoscope.set_shader_value(*loc_kal_segments, p.kal_segments);
        kaleidoscope.set_shader_value(*loc_kal_rotation, p.kal_rotation);
        kaleidoscope.set_shader_value(*loc_kal_mix, p.kal_mix);

        // Alto NEGATIVO: `rayos` es un RenderTexture y OpenGL numera las
        // filas de un framebuffer de abajo hacia arriba, al reves de como
        // raylib dibuja. Sin el signo la imagen sale de cabeza.
        //
        // Aca NO hace falta el rodeo por FFI del composite: el caleidoscopio
        // usa un solo sampler, y ese lo ata raylib solo con la textura que
        // se le pasa a `draw_texture_pro`.
        let origen = Rectangle {
            x: 0.0,
            y: 0.0,
            width: WIDTH as f32,
            height: -(HEIGHT as f32),
        };
        let destino = Rectangle {
            x: 0.0,
            y: 0.0,
            width: WIDTH as f32,
            height: HEIGHT as f32,
        };

        // Segundo sampler, con el mismo cuidado que en el composite: la
        // textura se ata DESPUES de `begin_shader_mode`, porque esa llamada
        // vacia el lote de dibujo de rlgl y con el la tabla de unidades de
        // textura. Atada antes, se pierde sin aviso y el shader lee la
        // textura nula: la mascara daria cero en todos lados y el efecto
        // desapareceria del todo, que se parece demasiado a "kMix esta en
        // cero". Se copia el handle antes de entrar porque el shader ya
        // queda prestado.
        let handle = *kaleidoscope.as_ref();
        let profundidad = *tex_rt.as_ref();
        let loc_prof = *loc_kal_depth;

        let fuente = rayos.texture();
        let mut tb = rl.begin_texture_mode(thread, plegada);
        let mut sm = tb.begin_shader_mode(kaleidoscope);
        unsafe { raylib::ffi::SetShaderValueTexture(handle, loc_prof, profundidad) };
        sm.draw_texture_pro(fuente, origen, destino, Vector2::zero(), 0.0, Color::WHITE);
    }

    /// Pasada final: aberracion cromatica y grano, ya sobre la ventana.
    ///
    /// Esta SI dibuja en pantalla: es la unica que no produce nada que haya
    /// que volver a leer.
    ///
    /// `tex_rt` es el cuadro crudo del trazador: de el se lee la
    /// profundidad para el desenfoque, y `foco` es la profundidad (en la
    /// escala del alpha, 0..1) que queda nitida.
    fn efectos<D: RaylibDraw>(
        &mut self,
        d: &mut D,
        tex_rt: &Texture2D,
        p: &SceneParams,
        tiempo: f32,
        foco: f32,
    ) {
        let PostGpu {
            effects,
            plegada,
            mips,
            loc_fx_bloom,
            loc_chromatic,
            loc_grain,
            loc_time,
            loc_focus,
            loc_dof,
            loc_fx_depth,
            loc_streak,
            loc_letterbox,
            ..
        } = self;

        effects.set_shader_value(*loc_chromatic, p.chromatic_aberration);
        effects.set_shader_value(*loc_grain, p.grain_amount);
        effects.set_shader_value(*loc_focus, foco);
        // La profundidad de campo se ABRE con la cancion: al principio el
        // fondo apenas se ablanda y al final la fuente queda recortada
        // contra un cielo desenfocado, como con una lente abierta del todo.
        effects.set_shader_value(*loc_dof, DOF_RADIO * (1.0 + p.cine * 0.9));
        // La estela se apaga de dia POR LO MISMO que el bloom (ver
        // `SceneParams::bloom_threshold`): sale del halo, asi que si el
        // halo crece con la escena iluminada la estela crece al cuadrado,
        // que es como el shader la pondera. De dia se comia el cuadro.
        effects.set_shader_value(
            *loc_streak,
            ESTELA_FUERZA * p.cine * (0.30 + 0.70 * (1.0 - p.luz_del_dia)),
        );
        effects.set_shader_value(*loc_letterbox, LETTERBOX_ALTO * p.cine);
        // El tiempo se acota antes de mandarlo. Con cero, `uv * time` da el
        // mismo punto para toda la pantalla y el grano sale plano; y despues
        // de unos minutos de cancion el numero se hace grande y el `sin` del
        // generador de ruido pierde precision, con lo que el grano se empieza
        // a ver en bandas. Un ciclo de un minuto arrancando en uno evita las
        // dos cosas y nadie nota que el patron se repita.
        effects.set_shader_value(*loc_time, 1.0 + tiempo.rem_euclid(60.0));

        // Alto negativo otra vez, por lo mismo: `plegada` es un
        // RenderTexture.
        let origen = Rectangle {
            x: 0.0,
            y: 0.0,
            width: WIDTH as f32,
            height: -(HEIGHT as f32),
        };
        let destino = Rectangle {
            x: 0.0,
            y: 0.0,
            width: WIDTH as f32,
            height: HEIGHT as f32,
        };

        // El segundo sampler se ata DESPUES de `begin_shader_mode`, por lo
        // mismo que en `componer` (ver ahi el porque).
        let handle = *effects.as_ref();
        let profundidad = *tex_rt.as_ref();
        let loc_prof = *loc_fx_depth;
        // Las estelas salen de un nivel GRUESO de la cadena, no del 0.
        // Ver `ESTELA_NIVEL`: es lo que evita que cada punto de luz deje
        // una fila de puntos en vez de una raya. Este nivel tiene la misma
        // paridad de volteo que el 0 (ver `armar_bloom`), asi que el shader
        // lo lee con la misma convencion de Y y no hay nada que cambiar
        // ahi.
        let halo = *mips[ESTELA_NIVEL.min(BLOOM_NIVELES - 1)].texture().as_ref();
        let loc_halo = *loc_fx_bloom;

        let fuente = plegada.texture();
        let mut sm = d.begin_shader_mode(effects);
        unsafe {
            raylib::ffi::SetShaderValueTexture(handle, loc_prof, profundidad);
            raylib::ffi::SetShaderValueTexture(handle, loc_halo, halo);
        }
        sm.draw_texture_pro(fuente, origen, destino, Vector2::zero(), 0.0, Color::WHITE);
    }
}

/// Radio maximo del desenfoque de profundidad de campo, en UV de la
/// ventana: 0.007 son unos 6 pixeles a 800 de ancho. Suave a proposito: el
/// fondo se ablanda, no se derrite.
const DOF_RADIO: f32 = 0.010;

/// Cuanto llegan a pesar las estelas anamorficas al final del tema.
///
/// SUBIDA de 3.2 a 6.0 al mismo tiempo que las estelas pasaron a salir de
/// un nivel grueso de la cadena de mips (ver `ESTELA_NIVEL`). No es que
/// antes estuviera mal afinada: cambio la fuente, y el numero solo tiene
/// sentido contra ella. El nivel 2 tiene la misma energia repartida sobre
/// sesenta y cuatro veces menos texeles, y como el shader eleva cada
/// muestra AL CUADRADO antes de sumarla, un punto de luz que ahi vale la
/// mitad aporta la cuarta parte. Medido sobre el climax, con el 3.2 viejo
/// la estela era invisible.
///
/// El tope de arriba lo pone la legibilidad y no el gusto: a 14 la raya del
/// altar es preciosa pero se COME la Triforce, que es el objeto que tiene
/// que quedar siempre visible. Se probo en 8 y aguantaba en los tramos
/// tranquilos, pero en el pico del tema —donde la piscina entera esta
/// encendida— la estela se sumaba a ese frente de luz y la inundacion
/// tapaba el altar igual. A 6 se lee como una raya de lente sobre el agua y
/// debajo del oro, y la Triforce sigue recortada contra ella incluso en el
/// segundo 142, que es el momento mas cargado de los tres minutos.
const ESTELA_FUERZA: f32 = 6.0;

/// De QUE NIVEL de la cadena de mips salen las estelas anamorficas.
///
/// Del 2 (100 x 75 para una ventana de 800 x 600), no del 0.
///
/// ES EL ARREGLO DE UN ARTEFACTO FEO. La estela se arma con unas pocas
/// muestras a lo largo de una linea horizontal, y para que esas muestras se
/// FUNDAN en una raya continua la imagen de la que salen tiene que ser
/// suave a la escala de la separacion entre ellas. El halo del gaussiano
/// viejo lo era. El de la cadena de mips NO: la cadena conserva un nucleo
/// nitido de pocos pixeles en cada punto de luz, que es justamente lo que
/// la hace bonita. Sacando las muestras de ahi, cada hada dejaba de dar una
/// raya y pasaba a dar una FILA DE PUNTOS separados, como un rosario; en el
/// climax, con decenas de puntos de luz a la vez, la pantalla entera se
/// llenaba de lineas punteadas y de un enrejado que no se parecia a nada
/// optico. Se ve clarisimo en cualquier captura del segundo 148.
///
/// El nivel 2 no tiene ese nucleo: un punto de luz de dos pixeles de la
/// pantalla ahi mide un cuarto de texel, o sea que ya viene repartido sobre
/// varios, y al estirarlo de vuelta a la pantalla la interpolacion bilineal
/// lo entrega como una mancha blanda de unos ocho pixeles. Sobre eso, las
/// muestras se funden solas.
///
/// Y ademas es lo correcto de por si: una estela anamorfica es un defecto
/// de la OPTICA, no del sensor. Sale de que la lente cilindrica dispersa la
/// luz en un eje, y lo que dispersa es el bulto del brillo, no su detalle
/// fino. Sacarla de una version gruesa del halo no es una aproximacion
/// barata: es lo que la hace parecerse a la de verdad.
const ESTELA_NIVEL: usize = 2;

/// Cuanto llegan a medir las bandas del formato ancho, en fraccion de la
/// altura. 0.11 arriba y abajo deja el cuadro en 2.35:1 desde el 4:3 de la
/// ventana, que es el formato ancho de cine.
const LETTERBOX_ALTO: f32 = 0.11;

/// El punto alrededor del que orbita la camara: el centro de la fuente, en
/// x y z. La altura a la que mira la pone la cancion (`camera_target_y`),
/// que se mueve apenas alrededor de 2.0.
const CAMARA_MIRA: Vec3 = Vec3::new(0.0, 2.0, 0.0);

/// Donde cae un punto del mundo en la pantalla, en UV de 0 a 1, con la
/// misma proyeccion que usa el trazador para tirar sus rayos (ver
/// `render_rows`: 45 grados de campo, aspecto de la ventana).
///
/// Es la inversa exacta de pixel -> rayo: se proyecta el punto sobre los
/// ejes de la camara y se divide por la profundidad. Fuera de pantalla (o
/// detras de la camara) se topa a los bordes: los god rays igual necesitan
/// un punto hacia donde caminar.
///
/// La `v` va con 1 ARRIBA: es el espacio de `fragTexCoord` cuando se dibuja
/// un RenderTexture con alto negativo, que es como se leen todos los
/// cuadros intermedios de la cadena.
fn proyectar_a_pantalla(camera: &Camera, punto: Vec3) -> [f32; 2] {
    let (right, up, forward) = camera.basis();
    let d = punto - camera.position;

    let fov = PI / 4.0;
    let scale = (fov / 2.0).tan();
    let aspect = WIDTH as f32 / HEIGHT as f32;

    // Detras de la camara: se empuja apenas adelante para que la division
    // no explote y el punto caiga en algun borde.
    let profundidad = dot(&d, &forward).max(1e-3);
    let ndc_x = dot(&d, &right) / profundidad / (aspect * scale);
    let ndc_y = dot(&d, &up) / profundidad / scale;

    [
        ((ndc_x + 1.0) / 2.0).clamp(0.0, 1.0),
        ((ndc_y + 1.0) / 2.0).clamp(0.0, 1.0),
    ]
}

/// Una camara parada en `ojo` mirando a `mira`, en los terminos de
/// `Camera` (posicion, yaw y pitch).
fn mirar_a(ojo: Vec3, mira: Vec3) -> Camera {
    let d = mira - ojo;
    // Con yaw = 0 y pitch = 0 la camara mira a -Z (ver `Camera`).
    let yaw = d.x.atan2(-d.z);
    let pitch = d.y.atan2((d.x * d.x + d.z * d.z).sqrt());
    Camera::new(ojo, yaw, pitch)
}

/// La camara orbital: siempre mirando a `CAMARA_MIRA`, parada a `radio` de
/// distancia, con `theta` girando alrededor y `phi` de altura.
///
/// NO da la vuelta entera: la fuente se mira de frente, y una orbita
/// completa la veria desde adentro de la pared del fondo. Es un PENDULO.
/// Sola, la camara oscila con un seno a los costados (y otro, mucho mas
/// chico, en altura), asi que desacelera suavemente en los extremos en vez
/// de rebotar; y las teclas se le SUMAN como un corrimiento manual. Los
/// dos juntos se topan en `THETA_MAX`: en el extremo el ojo queda en x =
/// 11 cos(0.25) sin(0.75) = 7.3, z = 11 cos(0.25) cos(0.75) = 7.8, siempre
/// delante de la fuente y lejos de la pared del fondo (z = -11).
struct Orbita {
    /// Segundos acumulados, la fase de los dos pendulos.
    timer: f32,
    /// Lo que sumaron las teclas al angulo horizontal, en radianes.
    theta_manual: f32,
    /// Lo que sumaron las teclas al angulo vertical, en radianes.
    phi_manual: f32,
    /// Distancia al punto de mira. Topada para no acercarse hasta adentro
    /// de los cristales ni alejarse hasta salir por la boca.
    radio: f32,
}

impl Orbita {
    /// Tope del angulo horizontal TOTAL (pendulo + teclas), a cada lado.
    const THETA_MAX: f32 = 0.75;
    /// Amplitud y periodo del pendulo horizontal.
    ///
    /// BAJADA de 0.65 a 0.36 radianes (de 37 a 21 grados) por una razon de
    /// encuadre, no de gusto. Las seis columnas estan en 0, 60, 120... y
    /// el eje de la camara apunta a 90 menos theta, asi que con el
    /// pendulo viejo el eje barria de 53 a 127 grados y se paraba encima
    /// de la columna de 60 y de la de 120 en cada extremo de la
    /// oscilacion: dos veces por ciclo, durante varios segundos, habia una
    /// columna JUSTO en el centro del cuadro tapando el altar. Se ve en
    /// cualquier foto vieja del segundo 87 o del 160.
    ///
    /// Con 0.36 el eje se queda entre 69 y 111 grados, o sea siempre
    /// dentro del hueco de 60 a 120, y las dos columnas de adelante pasan
    /// a hacer de MARCO a los costados en vez de tapar. Es mejor toma y
    /// ademas mas parecida a la original, donde a la fuente se la mira
    /// desde la boca de la cueva y no dando vueltas alrededor.
    const THETA_AMPLITUD: f32 = 0.36;
    const THETA_PERIODO: f32 = 34.0;
    /// El angulo vertical de reposo y su vaiven: entre 0.20 y 0.30, apenas
    /// perceptible, cada 18 segundos. Con un periodo distinto del
    /// horizontal el recorrido no se repite igual en cada ida y vuelta.
    /// Bajo a proposito: con 0.25 el ojo queda en y = 4.7, por DEBAJO de
    /// la linea del techo (5.0), asi que la fuente se mira por adentro,
    /// entre las columnas, y no desde arriba de las losas.
    const PHI_BASE: f32 = 0.25;
    const PHI_AMPLITUD: f32 = 0.05;
    const PHI_PERIODO: f32 = 23.0;
    /// Topes del angulo vertical total: ni desde abajo del piso ni desde
    /// la vertical, donde la camara se da vuelta.
    const PHI_MIN: f32 = 0.1;
    const PHI_MAX: f32 = 1.2;
    /// La distancia con la que arranca la camara.
    ///
    /// 10.7 y no 11.0: ver el comentario de `inicial`.
    const RADIO_INICIAL: f32 = 10.7;
    const RADIO_MIN: f32 = 5.0;
    const RADIO_MAX: f32 = 12.0;
    /// La camara RESPIRA: el radio oscila +/- 1.2 cada 41 segundos, un
    /// dolly lentisimo que acerca y aleja la fuente. Con un periodo que no
    /// divide a los otros dos, el recorrido no se repite nunca igual.
    const RADIO_AMPLITUD: f32 = 1.2;
    const RADIO_PERIODO: f32 = 41.0;
    /// Cuanto se acerca la camara entre el principio y el final del tema.
    const CINE_ACERCAMIENTO: f32 = 1.5;
    // LA CAMARA YA NO SE EMPUJA CON EL GOLPE.
    //
    // Habia un `PULSO_EMPUJE` que la adelantaba unos centimetros en cada
    // tiempo fuerte, con la idea de que el golpe se sintiera en el cuerpo.
    // Se saco: el tema es una nana, la escena es un lugar en calma, y una
    // camara que da un tironcito cada medio segundo no acompana ese clima
    // sino que pone al que mira en tension. Lo que tiene que latir es la
    // LUZ, no el punto de vista.
    //
    // El pulso sigue moviendo el bloom, las luces de la fuente, las hadas
    // y los haces de luz. Eso se siente y no inquieta, porque son cosas
    // que estan pasando adentro de la cueva y no sacudidas del encuadre.

    /// De frente, apenas elevada (~14 grados) y a 11 de distancia: el ojo
    /// queda en (0, 4.7, 10.7), bajo el techo, con la fuente en cuadro.
    fn inicial() -> Self {
        Orbita {
            timer: 0.0,
            theta_manual: 0.0,
            phi_manual: 0.0,
            // 10.7 y no 11.0: la respiracion del radio es de +/- 1.2 y el
            // tope de arriba esta en 12. Arrancando en 11, la suma llegaba
            // a 12.2 y se topaba, asi que durante un 22% de cada ciclo de
            // cuarenta y un segundos el dolly se FRENABA EN SECO contra el
            // limite y despues arrancaba de nuevo. Eso no se lee como una
            // camara que respira sino como una que se traba: es la otra
            // mitad de lo que se reportaba como composicion abrupta.
            //
            // Con 10.7 el recorrido entero (9.5 a 11.9) cabe adentro del
            // rango y la respiracion nunca toca el tope. El encuadre queda
            // un 3% mas cerca, que no se nota.
            radio: Self::RADIO_INICIAL,
        }
    }

    /// Donde esta el pendulo horizontal en este instante.
    fn theta_auto(&self) -> f32 {
        (self.timer * 2.0 * PI / Self::THETA_PERIODO).sin() * Self::THETA_AMPLITUD
    }

    /// Y el vertical.
    fn phi_auto(&self) -> f32 {
        Self::PHI_BASE + (self.timer * 2.0 * PI / Self::PHI_PERIODO).sin() * Self::PHI_AMPLITUD
    }

    /// La respiracion del radio en este instante.
    fn radio_auto(&self) -> f32 {
        (self.timer * 2.0 * PI / Self::RADIO_PERIODO).sin() * Self::RADIO_AMPLITUD
    }

    /// Distancia total al punto de mira: la manual mas la respiracion,
    /// topada.
    fn distancia(&self) -> f32 {
        (self.radio + self.radio_auto()).clamp(Self::RADIO_MIN, Self::RADIO_MAX)
    }

    /// Angulo horizontal total, topado.
    fn theta(&self) -> f32 {
        (self.theta_auto() + self.theta_manual).clamp(-Self::THETA_MAX, Self::THETA_MAX)
    }

    /// Angulo vertical total, topado.
    fn phi(&self) -> f32 {
        (self.phi_auto() + self.phi_manual).clamp(Self::PHI_MIN, Self::PHI_MAX)
    }

    /// Avanza el pendulo `dt` segundos y suma lo que pidieron las teclas.
    ///
    /// El corrimiento manual se topa contra el mismo limite que el total:
    /// si no, manteniendo una flecha apretada contra el tope el
    /// corrimiento seguiria creciendo sin que se vea nada, y despues habria
    /// que apretar la otra el mismo tiempo antes de que la camara volviera
    /// a moverse.
    fn avanzar(&mut self, dt: f32, d_theta: f32, d_phi: f32, d_radio: f32) {
        self.timer += dt;

        let auto = self.theta_auto();
        self.theta_manual =
            (self.theta_manual + d_theta).clamp(-Self::THETA_MAX - auto, Self::THETA_MAX - auto);

        let auto = self.phi_auto();
        self.phi_manual =
            (self.phi_manual + d_phi).clamp(Self::PHI_MIN - auto, Self::PHI_MAX - auto);

        self.radio = (self.radio + d_radio).clamp(Self::RADIO_MIN, Self::RADIO_MAX);
    }

    /// A que distancia esta el centro de la fuente, SIN contar el empujon
    /// del beat: es la distancia a la que hay que enfocar.
    fn distancia_de_foco(&self, cine: f32) -> f32 {
        (self.distancia() - Self::CINE_ACERCAMIENTO * cine)
            .clamp(Self::RADIO_MIN, Self::RADIO_MAX)
    }

    /// La camara de este cuadro, mirando a la altura `mira_y`.
    ///
    /// `cine` (de 0 a 1) la ACERCA hasta `CINE_ACERCAMIENTO` unidades: al
    /// final del tema la fuente llena mas el cuadro. Va aca y no sumado a
    /// `radio` para que no se acumule con las teclas: es un corrimiento
    /// del encuadre, no algo que el usuario este pidiendo.
    ///
    fn camara_cine(&self, mira_y: f32, cine: f32) -> Camera {
        let (theta, phi) = (self.theta(), self.phi());
        let radio = (self.distancia() - Self::CINE_ACERCAMIENTO * cine)
            .clamp(Self::RADIO_MIN, Self::RADIO_MAX);
        let ojo = Vec3::new(
            CAMARA_MIRA.x + radio * phi.cos() * theta.sin(),
            CAMARA_MIRA.y + radio * phi.sin(),
            CAMARA_MIRA.z + radio * phi.cos() * theta.cos(),
        );
        mirar_a(ojo, Vec3::new(CAMARA_MIRA.x, mira_y, CAMARA_MIRA.z))
    }

    /// La camara de este cuadro, mirando a la altura `mira_y`.
    fn camara(&self, mira_y: f32) -> Camera {
        let (theta, phi) = (self.theta(), self.phi());
        let radio = self.distancia();
        let ojo = Vec3::new(
            CAMARA_MIRA.x + radio * phi.cos() * theta.sin(),
            CAMARA_MIRA.y + radio * phi.sin(),
            CAMARA_MIRA.z + radio * phi.cos() * theta.cos(),
        );
        mirar_a(ojo, Vec3::new(CAMARA_MIRA.x, mira_y, CAMARA_MIRA.z))
    }
}

/// El termino `i` de la sucesion de Halton en base `base`, en [0, 1).
///
/// Es la sucesion de baja discrepancia de siempre: cada termino cae en el
/// hueco mas grande que dejaron los anteriores, asi que ocho muestras
/// seguidas cubren el pixel pareja y no se amontonan como lo harian ocho
/// numeros al azar. Con Halton en base 2 para X y base 3 para Y, el
/// recorrido del jitter no repite un patron visible.
fn halton(mut i: u32, base: u32) -> f32 {
    let mut f = 1.0f32;
    let mut r = 0.0f32;
    while i > 0 {
        f /= base as f32;
        r += f * (i % base) as f32;
        i /= base;
    }
    r
}

/// Cuantos pixeles se movio la imagen entre dos camaras.
///
/// No se estima con el angulo: se PROYECTAN las ocho esquinas de una caja
/// que envuelve la fuente con las dos camaras y se mide el corrimiento mas
/// grande. Asi se contempla igual el giro que el acercamiento, que mueve
/// los bordes mucho mas que el centro.
///
/// Es lo que decide cuanto vale la historia del acumulador temporal: por
/// debajo de un pixel, el cuadro anterior sigue siendo una muestra del
/// mismo punto de vista; por encima, es otra imagen.
fn movimiento_en_pixeles(antes: &Camera, ahora: &Camera) -> f32 {
    let mut peor = 0.0f32;
    for dx in [-4.0f32, 4.0] {
        for dy in [0.0f32, 5.5] {
            for dz in [-4.0f32, 4.0] {
                let p = Vec3::new(dx, dy, dz);
                let a = proyectar_a_pantalla(antes, p);
                let b = proyectar_a_pantalla(ahora, p);
                let d = (((a[0] - b[0]) * WIDTH as f32).powi(2)
                    + ((a[1] - b[1]) * HEIGHT as f32).powi(2))
                .sqrt();
                peor = peor.max(d);
            }
        }
    }
    peor
}

/// Cuanto pesa el cuadro recien trazado cuando la camara esta quieta: un
/// cuarto, o sea que la imagen converge sobre las ultimas ~4 muestras.
/// Menos que esto arrastra demasiado y las hadas dejan estela aunque el
/// recorte de vecindad la corte.
const TAA_PESO_MINIMO: f32 = 0.25;

/// A partir de cuantos pixeles de movimiento la historia deja de servir y
/// el cuadro se muestra crudo.
///
/// Alto porque la historia se REPROYECTA (ver `Framebuffer::acumular`): el
/// movimiento de la camara ya esta contemplado, y esto solo ataja los
/// saltos. El pendulo mueve 2.6 pixeles por cuadro (medido con `--taa`),
/// asi que con este umbral no lo toca; arrastrando el mouse fuerte, si.
const TAA_MOVIMIENTO_MAXIMO: f32 = 25.0;

/// Cuantas vueltas espera el modo foto antes de disparar, para que el
/// acumulador temporal converja. Con peso 0.25 por cuadro, a las doce
/// muestras lo que quedaba del primer cuadro pesa menos de un nivel de
/// 255.
const FOTO_CUADROS: u32 = 14;

/// Indice de la luna en la lista de luces.
const LUZ_LUNA: usize = 5;

/// Indices de las dos antorchas.
const LUZ_ANTORCHAS: [usize; 2] = [6, 7];

/// Ruido suave en [0, 1] a partir del tiempo: se sortea un valor por
/// escalon y se interpola con una curva en S. Es lo que hace que la llama
/// PARPADEE en vez de vibrar: el ruido por cuadro se ve como un error de
/// dibujado, y este sube y baja con cuerpo.
fn llama(t: f32, semilla: u32) -> f32 {
    let escalon = t.floor();
    let f = t - escalon;
    let f = f * f * (3.0 - 2.0 * f);
    let v = |n: f32| {
        let mut h = (n as i32 as u32).wrapping_mul(374_761_393) ^ semilla.wrapping_mul(668_265_263);
        h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
        ((h ^ (h >> 16)) & 0xFFFF) as f32 / 65535.0
    };
    let a = v(escalon);
    a + (v(escalon + 1.0) - a) * f
}

/// La noche avanza: el cielo gira, el amanecer sube, y la luz de la luna
/// sigue a la luna del cielo y se calienta con el alba. Se llama una vez
/// por cuadro, antes de trazar, con los parametros de ese instante.
fn avanzar_noche(cielo: &mut Cielo, lights: &mut [Light], p: &SceneParams) {
    // `Cielo` solo sabe de resplandor del horizonte, y cuanto resplandor
    // hay es exactamente cuanta luz de dia hay.
    cielo.ajustar(p.giro_cielo, p.luz_del_dia, p.tiempo, &p.estrellas, p.swell);

    if let Some(luna) = lights.get_mut(LUZ_LUNA) {
        luna.position = cielo.luna() * 40.0;
        // LA MISMA LUZ HACE DE SOL Y DE LUNA, y es lo que ata el arco
        // entero a un solo numero. Con luz de dia es un oro rosado y
        // fuerte, que es el sol bajo del amanecer; sin ella se enfria a
        // blanco azulado y se cae a menos de la mitad, que es la luna. Y
        // como esta luz ya SEGUIA a la del cielo (misma direccion, ver
        // arriba), el sol y la luna salen y se ponen por donde
        // corresponde sin ninguna cuenta aparte.
        let a = p.luz_del_dia;
        luna.color = color_f(0.75 + a * 0.25, 0.8 - a * 0.15, 1.0 - a * 0.45);
        luna.intensity = 0.4 + a * 0.5;
    }

    // Las antorchas parpadean. Cada una con su propia semilla y a su
    // propio ritmo, porque dos llamas sincronizadas se leen como un efecto
    // y no como fuego. El latido de la cancion las aviva apenas: el fuego
    // responde al tema, pero de lejos.
    for (n, &i) in LUZ_ANTORCHAS.iter().enumerate() {
        if let Some(antorcha) = lights.get_mut(i) {
            let ruido = llama(p.tiempo * 3.3 + n as f32 * 11.0, 7 + n as u32 * 31);
            antorcha.intensity = 2.4 * (0.72 + ruido * 0.38 + p.pulso * 0.12);
        }
    }
}

fn main() {
    // OJO con el orden: la ventana se abre DESPUES de construir la escena.
    //
    // Nada de lo que viene abajo (materiales, geometria, luces, audio)
    // necesita un contexto grafico: las texturas se generan por codigo y los
    // Color de raylib son datos y nada mas. Lo unico que pide ventana son las
    // texturas de video, ya sobre el final.
    //
    // Se hace asi para que el trazado se pueda medir y volcar a PNG en una
    // maquina sin pantalla disponible. Con la init arriba, si GLFW no
    // consigue monitor el programa se muere antes de construir un solo
    // objeto, y ahi no se puede ni depurar la escena.

    // ============================================================
    //  MATERIALES
    // ============================================================
    //
    // Cinco con textura de `resources/textures/` mas el de los orbes, que
    // es color puro. El "albedo" de la consigna es un color por canal: aca
    // entra como el TINTE de la textura (se multiplica canal por canal
    // sobre la imagen) y el primer peso del material queda en cuanto de
    // ese color responde a las luces. Los otros tres pesos son especular,
    // reflexion y refraccion.

    let cargar = |archivo: &str| Arc::new(TextureImage::load(&format!("{TEXTURAS}/{archivo}")));

    // --- RELIEVES ---
    // Mapas de normales generados por codigo (ver
    // `TextureImage::relieve_procedural`): la luz los ve, la geometria no
    // los tiene. Cada material lleva el suyo con su propia escala: la
    // piedra es rugosa a lo grande, el marmol tiene vetas finas, la
    // obsidiana esta astillada y el oro martillado.
    let relieve_piedra = Arc::new(TextureImage::relieve_procedural(256, 5, 5, 3.0, 1));
    let relieve_marmol = Arc::new(TextureImage::relieve_procedural(256, 7, 4, 0.5, 2));
    let relieve_obsidiana = Arc::new(TextureImage::relieve_procedural(128, 4, 3, 0.4, 3));
    // El martillado del oro. Tres cambios sobre lo que era (128, 12, 3,
    // 0.7): el doble de resolucion, MENOS octavas y menos fuerza.
    //
    // Es contra el CENTELLEO DEL ESPECULAR, que es lo que hacia que las
    // molduras se vieran como arena naranja. El brillo especular del oro va
    // con exponente alto, o sea un lobulo angosto: la superficie refleja
    // fuerte en una direccion muy precisa. Si la normal cambia mucho entre
    // un pixel y el vecino —que es lo que hace un relieve de tres octavas,
    // cuya ultima octava tiene detalle del tamano de un texel— cada pixel
    // acierta o falla ese lobulo por su cuenta, y el resultado no es una
    // superficie rugosa: es ruido blanco. El trazador toma UNA muestra por
    // pixel, asi que no hay nada que lo promedie.
    //
    // Dos octavas dejan el bulto del martillado (que es lo que se ve) y
    // tiran la octava que solo aportaba centelleo.
    let relieve_oro = Arc::new(TextureImage::relieve_procedural(256, 9, 2, 0.45, 4));

    // --- 1. PIEDRA DE CUEVA ---
    // El fondo: piso y paredes lejanas, mate. Con albedo suficiente para
    // que la poca luz que las toca se note: en la fuente original la cueva
    // se ve, no es un vacio negro.
    let piedra = Material::new(
        [1.0, 0.05, 0.0, 0.0],
        6.0,
        0.0,
        Texture::ImageTexture(cargar("stone_cave.png"), color_f(0.40, 0.40, 0.55), (0.0, 0.0)),
        None,
    )
    .con_relieve(relieve_piedra.clone(), 1.0);

    // --- 2. MARMOL DE HADA ---
    // La piedra teal de la fuente: bordes de la piscina, columnas, techo.
    //
    // SIN emision, a proposito. La consigna le daba un brillo propio de
    // (0.02, 0.04, 0.04), pero en este engine lo que brilla solo NO tapa
    // la luz (ver `puede_tapar`), y con eso el techo y las columnas
    // dejaban de dar sombra: la cyan cenital atravesaba las losas como si
    // no estuvieran. Y ese brillo eran 5 de 255 por canal, menos de lo
    // que ya le pone el ambiente cyan a la piedra en la sombra. Se pierde
    // nada y se gana el techo.
    let marmol = Material::new(
        [1.0, 0.22, 0.1, 0.0],
        18.0,
        0.0,
        Texture::ImageTexture(cargar("fairy_marble.png"), color_f(0.70, 0.60, 0.65), (0.0, 0.0)),
        None,
    )
    .con_relieve(relieve_marmol.clone(), 1.0)
    .con_rugosidad(0.10);

    // El marmol de la estela del fondo: el mismo, SIN reflexion.
    //
    // No es una decision de estilo, es de costo: la estela es una
    // superficie grande y lejana, y con el peso de reflexion del marmol
    // (0.1, que Fresnel sube mas todavia a angulo rasante) cada uno de
    // sus pixeles disparaba un rayo de rebote. Se midio: pasaba el cuadro
    // de 44 a 50 milisegundos para devolver un reflejo del cielo que a esa
    // distancia y detras de la niebla no se ve. El brillo especular se
    // queda, que es lo que hace que la piedra se lea pulida.
    // OSCURECIDO, y es una correccion de composicion, no de material.
    //
    // La estela es una losa de cuatro por seis puesta detras del altar: es
    // el TELON DE FONDO del sujeto. Con el tinte de antes (0.62, 0.55,
    // 0.60) y albedo pleno era la superficie grande MAS CLARA del cuadro,
    // mas que las columnas que estan al doble de cerca, y eso rompia dos
    // cosas a la vez.
    //
    // La primera es de lectura: la Triforce del altar tiene que recortarse
    // CONTRA la estela, y un sujeto claro sobre un fondo igual de claro no
    // se recorta contra nada.
    //
    // La segunda es que reventaba el bloom. El umbral del bloom mira la
    // luminancia pixel por pixel, no le importa que sea un objeto grande y
    // plano: la estela entera lo cruzaba, y cuatro por seis unidades de
    // superficie encendida entran a la cadena de mips como una MANCHA, no
    // como un punto de luz. Al subir la cadena esa mancha se convertia en
    // un globo blanco de medio cuadro justo detras de la Triforce, que es
    // el peor lugar posible. Se veia en cualquier captura del climax.
    //
    // Un halo bonito sale de algo CHICO y muy brillante. De algo grande y
    // medianamente brillante sale niebla.
    let marmol_mate = Material::new(
        [0.72, 0.30, 0.0, 0.0],
        30.0,
        0.0,
        Texture::ImageTexture(cargar("fairy_marble.png"), color_f(0.32, 0.29, 0.36), (0.0, 0.0)),
        None,
    )
    .con_relieve(relieve_marmol.clone(), 1.0);

    // El fondo de la piscina: teal profundo y mate, sin brillo.
    let marmol_fondo = Material::new(
        [0.8, 0.05, 0.0, 0.0],
        10.0,
        0.0,
        Texture::ImageTexture(cargar("fairy_marble.png"), color_f(0.30, 0.45, 0.50), (0.0, 0.0)),
        None,
    )
    .con_relieve(relieve_marmol.clone(), 1.0)
    // El fondo de la piscina es el unico que lleva causticas: es el que
    // tiene agua encima. Ver `Material::causticas`.
    .con_causticas(1.3);

    // El marmol del piso de la plaza: el mismo, pero PULIDO. Mas
    // reflexion, y Fresnel la lleva a espejo a angulo rasante: la fuente
    // entera se refleja en el piso mojado alrededor de la piscina.
    let marmol_pulido = Material::new(
        [0.9, 0.6, 0.30, 0.0],
        80.0,
        0.0,
        Texture::ImageTexture(cargar("fairy_marble.png"), color_f(0.55, 0.50, 0.58), (0.0, 0.0)),
        None,
    )
    .con_relieve(relieve_marmol.clone(), 1.0)
    // La losa de la plaza es el peor caso de la escena: es la superficie
    // grande que refleja la fuente entera, y con reflexion perfecta se leia
    // como una lamina de vidrio puesta sobre la piedra. Piedra pulida y
    // humeda refleja, pero su reflejo esta ESTIRADO y desenfocado.
    .con_rugosidad(0.13);

    // --- 3. AGUA ---
    // El corazon visual: en la fuente original el agua ILUMINA todo desde
    // abajo. Quieta como un espejo, refleja la mitad, deja pasar el resto
    // hasta el fondo de marmol, y brilla fuerte: el bloom convierte esa
    // emision en un resplandor cyan que bania las columnas y el techo.
    let agua = Material::new(
        [0.25, 0.3, 0.5, 0.6],
        120.0,
        1.33,
        Texture::ImageTexture(cargar("water_fairy.png"), color_f(0.25, 0.55, 0.65), (0.0, 0.0)),
        // Bajada de (0.05, 0.15, 0.18) con el post-procesado en lineal: con
        // Fresnel y el bloom sumando, la piscina entera se iba a cyan
        // reventado.
        Some(color_f(0.04, 0.11, 0.14)),
    )
    // Poca: el agua quieta es casi un espejo. Lo justo para que el borde
    // entre el cielo reflejado y el agua deje de ser un recorte con filo
    // de tijera, que es lo que la hacia parecer una lamina de plastico.
    .con_rugosidad(0.030);

    // --- 4. ORO ---
    // Molduras y Triforce. El de las molduras lleva la textura del mineral;
    // el de la Triforce es el mismo color PLANO (el oro liso se lee mas
    // iconico en los tres triangulos) con la emision al doble: es el
    // segundo punto focal despues del agua.
    let oro = Material::new(
        [1.0, 0.95, 0.35, 0.0],
        // Exponente especular bajado de 150 a 95, por lo mismo que el
        // relieve: un lobulo mas ancho reparte el reflejo entre mas
        // pixeles y deja de encenderse y apagarse de a uno. El oro sigue
        // leyendose pulido —95 es un lobulo estrecho igual— y ademas ahora
        // el reflejo se ve como una FRANJA a lo largo de la moldura, que es
        // como se ve el brillo en una pieza de metal larga, en vez de como
        // puntos sueltos.
        95.0,
        0.0,
        Texture::ImageTexture(cargar("gold_triforce.png"), color_f(0.85, 0.7, 0.2), (0.0, 0.0)),
        Some(color_f(0.3, 0.25, 0.06)),
    )
    .con_relieve(relieve_oro.clone(), 1.0)
    // El oro esta MARTILLADO y cepillado: su textura son abolladuras y
    // rayas de lima, asi que su reflejo tiene que estar estirado y roto.
    // Con reflejo perfecto las molduras parecian cromadas.
    .con_rugosidad(0.13);
    let oro_sagrado = Material::new(
        oro.albedo,
        oro.specular,
        0.0,
        Texture::Solid(color_f(0.85, 0.7, 0.2)),
        Some(color_f(0.7, 0.55, 0.15)),
    );

    // El oro del emblema del muro: el mismo, mas apagado y con brillo
    // propio mas bajo. Esta a diez unidades de la camara y detras de la
    // niebla; con el oro de las molduras competia con la Triforce del
    // altar, que es la que tiene que ganar.
    let oro_muro = Material::new(
        [0.9, 0.5, 0.12, 0.0],
        90.0,
        0.0,
        Texture::Solid(color_f(0.62, 0.50, 0.20)),
        Some(color_f(0.20, 0.16, 0.05)),
    );

    // --- 5. OBSIDIANA ---
    // Acentos oscuros y muy reflectantes: el pedestal. Su color propio
    // casi no se ve; lo que se ve en ella son las hadas y el agua.
    let obsidiana = Material::new(
        [1.0, 0.7, 0.6, 0.0],
        200.0,
        0.0,
        Texture::ImageTexture(cargar("obsidian.png"), color_f(0.06, 0.04, 0.10), (0.0, 0.0)),
        None,
    )
    .con_relieve(relieve_obsidiana.clone(), 1.0)
    // Vidrio volcanico: de las superficies mas pulidas que hay en la
    // naturaleza, pero no un espejo. Apenas la justa.
    .con_rugosidad(0.045);

    // --- 6. CRISTAL ---
    // Los cristales que crecen en las esquinas de la plaza. Refractan
    // como vidrio (1.5), reflejan bastante y Fresnel reparte entre las
    // dos segun el angulo. Sin emision: lo que brilla es la luz de hada
    // que llevan ADENTRO (una esferita emisiva), vista a traves del
    // cristal. Que no sean emisivos importa: asi tapan la luz, y como su
    // peso de refraccion es alto, la sombra que dan es tenue y de color.
    let cristal = Material::new(
        [0.35, 0.9, 0.25, 0.7],
        120.0,
        1.5,
        Texture::ImageTexture(cargar("crystal.png"), color_f(0.8, 0.6, 0.95), (0.0, 0.0)),
        None,
    )
    // El cuarzo tiene caras planas y pulidas: casi espejo.
    .con_rugosidad(0.022);
    // --- 7. RUPIAS ---
    // La gema de Hyrule: refracta como el cristal pero mas densa (1.6,
    // como una esmeralda), con brillo propio del color que le toque. Tres
    // colores, que son los tres primeros que uno encuentra en el juego:
    // verde, azul y rojo.
    let rupia = |tinte: Color, emision: Color| {
        Material::new(
            [0.3, 1.0, 0.30, 0.55],
            180.0,
            1.6,
            Texture::Solid(tinte),
            Some(emision),
        )
    };

    // --- 8. FUEGO DE ANTORCHA ---
    // Naranja caliente y emisivo: es lo unico calido de toda la escena, y
    // por eso funciona. Todo lo demas es teal, rosa y violeta; dos puntos
    // de fuego en la entrada le dan al cuadro un contraste de temperatura
    // que ninguna cantidad de bloom puede fabricar.
    let fuego = Material::new(
        [1.0, 0.0, 0.0, 0.0],
        1.0,
        0.0,
        Texture::Solid(color_f(1.0, 0.75, 0.4)),
        Some(color_f(1.0, 0.55, 0.15)),
    );

    // La luz que vive ADENTRO de cada cristal. Cuatro colores, uno por
    // racimo, porque cada uno se queda con un tercio del circulo de
    // quintas (ver `animacion::energia_region`): teniendo cada familia
    // armonica su propio color, cuando el tema modula no solo cambia de
    // esquina la luz, cambia el color de la plaza.
    //
    // Se recorren en el orden de la rueda de color y no al azar: violeta,
    // rosa, ambar y verde son los cuatro puntos que mas se distinguen
    // entre si sobre el teal de la fuente.
    const CRISTAL_COLORES: [(f32, f32, f32); 4] = [
        (0.75, 0.40, 1.00),
        (1.00, 0.40, 0.75),
        (1.00, 0.70, 0.35),
        (0.40, 1.00, 0.70),
    ];
    let luz_de_cristal = |c: (f32, f32, f32)| {
        Material::new(
            [1.0, 0.0, 0.0, 0.0],
            1.0,
            0.0,
            Texture::Solid(color_f(1.0, 0.9, 1.0)),
            Some(color_f(c.0, c.1, c.2)),
        )
    };

    // --- ORBES DE HADA ---
    // Rosa palido con emision alta: son luces, no objetos. Sin textura.
    // La emision vive en 8 bits y el halo del bloom sale del color del
    // pixel, asi que solo un canal puede saturar: en el rosa domina el R
    // (255, 102, 230) y en el cyan el G y el B (77, 255, 255). Con mas
    // emision se van a blanco y el halo pierde el color. Lo que agranda
    // el halo no es la emision sino el RADIO: mas pixeles sobre el umbral.
    let orbe = |emision: Color| {
        Material::new(
            [1.0, 0.0, 0.0, 0.0],
            1.0,
            0.0,
            Texture::Solid(color_f(1.0, 0.8, 0.9)),
            Some(emision),
        )
    };

    // ============================================================
    //  GEOMETRIA
    // ============================================================
    //
    // La fuente esta centrada en el origen, con Y arriba y Z hacia la
    // camara. Es la Great Fairy Fountain de Ocarina of Time: una piscina
    // cuadrada de marmol teal con seis columnas alrededor, un techo con el
    // centro abierto, molduras de oro, y en el medio del agua un pedestal
    // de obsidiana con la Triforce, rodeada de hadas. Todo adentro de una
    // cueva oscura que apenas se ve.
    //
    // Casi todo son cuboides (`Cube::new_rect`); las columnas son
    // cilindros, el agua un plano recortado, las hadas esferas y la
    // Triforce tres triangulos.

    let mut objects: Vec<Box<dyn RayIntersect + Send + Sync>> = Vec::new();

    // Lo que la cancion va a poder tocar despues. Se anota A MEDIDA que se
    // construye: el indice de un objeto es donde cae en esta lista.
    let mut escena = EscenaViva::nueva();

    /// Un cuboide centrado en `(x, y, z)` de `sx` por `sy` por `sz`, con la
    /// textura en UNIDADES DEL MUNDO.
    ///
    /// EL MOSAICO NO ES DECORACION, ARREGLA UN DEFECTO. Sin el, `Cube` mapea
    /// una copia entera de la textura sobre CADA CARA, con las UV
    /// normalizadas de 0 a 1 sobre lo que mida la cara. En un cubo eso esta
    /// bien; en las piezas de la fuente, que son molduras y losas, es un
    /// desastre: la moldura de oro del borde de la piscina mide 5.5 por
    /// 0.3, asi que la textura sale estirada dieciocho veces en un eje y
    /// comprimida en el otro.
    ///
    /// Y no queda solo feo. El mapa de relieve se muestrea sobre esas
    /// MISMAS UV deformadas, asi que la normal pega saltos de un pixel al
    /// siguiente a lo largo del eje comprimido; con el brillo especular del
    /// oro en 150, cada pixel acierta o falla el reflejo por separado y la
    /// moldura sale como granizo naranja en vez de como metal. Se ve en
    /// cualquier captura vieja de la piscina.
    ///
    /// Con las UV en unidades del mundo, un metro de moldura mide lo mismo
    /// en los dos ejes y en todas las piezas: el relieve vuelve a ser
    /// relieve, y de paso el marmol —que trae una moldura tallada en el
    /// borde de la textura— repite una vez por unidad y las losas largas se
    /// leen como sillares puestos uno al lado del otro en vez de como una
    /// unica piedra imposible de ocho metros.
    const MOSAICO: f32 = 1.0;
    fn bloque(x: f32, y: f32, z: f32, sx: f32, sy: f32, sz: f32, material: &Material) -> Box<Cube> {
        Box::new(
            Cube::new_rect(Vec3::new(x, y, z), sx, sy, sz, material.clone())
                .con_mosaico(MOSAICO),
        )
    }

    /// Un cubo centrado en `(x, y, z)` de lado `lado`.
    fn cubo(x: f32, y: f32, z: f32, lado: f32, material: &Material) -> Box<Cube> {
        Box::new(Cube::new(Vec3::new(x, y, z), lado, material.clone()))
    }

    // --- 1. LA CUEVA ---
    // Un piso enorme y tres paredes bajas de piedra oscura (fondo y
    // costados), con mosaico para que la piedra se lea como piedra y no
    // como una mancha estirada 30 unidades. Van sueltos: son cuatro
    // cuboides y los prueba todo rayo de todos modos.
    //
    // La pared del fondo es BAJA a proposito (llega a y = 3.2): la fuente
    // esta en la boca de la cueva, abierta al cielo, y por encima del
    // borde de roca se ve la noche, la nebulosa y la luna. Con la pared
    // hasta el techo el skybox no aparecia en ningun cuadro.
    let roca = |x: f32, y: f32, z: f32, sx: f32, sy: f32, sz: f32| {
        Box::new(Cube::new_rect(Vec3::new(x, y, z), sx, sy, sz, piedra.clone()).con_mosaico(0.22))
    };
    objects.push(roca(0.0, -1.0, 0.0, 30.0, 1.0, 30.0));
    // La pared del fondo y, contra ella, LA ESTELA: una losa alta con el
    // emblema grabado. Van juntas en un grupo porque la pared ya mide
    // treinta unidades y su esfera acotante cubre a la estela sin
    // crecer, asi que agruparlas sale gratis y deja la lista de los que
    // pueden tapar la luz del mismo largo (ver el comentario de la
    // entrada sobre por que eso importa).
    objects.push(Box::new(GrupoAcotado::new(vec![
        roca(0.0, 1.1, -12.0, 30.0, 4.2, 2.0),
        // La estela sobresale POR ENCIMA del muro (que llega a 3.2) a
        // proposito: asi su mitad de arriba, que es donde esta el
        // emblema, se recorta contra el cielo en vez de perderse contra
        // la roca negra. Y es de marmol y no de piedra de cueva, para que
        // se lea como algo puesto ahi por alguien.
        Box::new(
            Cube::new_rect(Vec3::new(0.0, 2.0, -10.6), 4.2, 6.2, 0.6, marmol_mate.clone())
                .con_mosaico(0.35),
        ),
    ])));
    objects.push(roca(-14.0, 0.8, 0.0, 2.0, 3.6, 30.0));
    objects.push(roca(14.0, 0.8, 0.0, 2.0, 3.6, 30.0));

    // La plaza: una losa de marmol pulido alrededor de la piscina, con las
    // baldosas repitiendo cada dos unidades. Es el espejo en el que se
    // refleja la fuente entera.
    objects.push(Box::new(
        Cube::new_rect(Vec3::new(0.0, -0.45, 0.0), 12.0, 0.1, 12.0, marmol_pulido.clone())
            .con_mosaico(0.5),
    ));


    // --- 2. LA PISCINA ---
    // Cuadrada, de 6 x 6, con cuatro bordes de marmol y el fondo hundido.
    // (Lo octogonal de la fuente original se aproximaba con escaleras de
    // cubitos en las esquinas y se veia peor que el cuadrado limpio.)
    const PISCINA: f32 = 3.0;
    let mut piscina: Vec<Box<dyn RayIntersect + Send + Sync>> = vec![
        bloque(0.0, 0.25, PISCINA, 6.0, 0.8, 0.6, &marmol),
        bloque(0.0, 0.25, -PISCINA, 6.0, 0.8, 0.6, &marmol),
        bloque(-PISCINA, 0.25, 0.0, 0.6, 0.8, 6.0, &marmol),
        bloque(PISCINA, 0.25, 0.0, 0.6, 0.8, 6.0, &marmol),
        // El fondo, visible a traves del agua: marmol mas oscuro y mate,
        // para que lo que se vea a traves del agua sea el agua y no una
        // losa blanca iluminada.
        //
        // El mosaico subio de 0.7 a 1.6 repeticiones por unidad. Con 0.7
        // una repeticion de la textura medi­a metro y medio de mundo,
        // asi que la veta del marmol salia AMPLIFICADA diez veces y lo
        // que se veia por debajo del agua no eran baldosas: era un
        // remolino azul gigante, el unico objeto psicodelico de una
        // escena que no lo es. A 1.6 cada baldosa mide 0.6 y el fondo se
        // lee como el fondo embaldosado de una piscina, que ademas es lo
        // que le da ESCALA a la piscina: sin un patron repetido de
        // tamano conocido, un cuadrado de color no dice si mide dos
        // metros o veinte.
        Box::new(
            Cube::new_rect(Vec3::new(0.0, -0.3, 0.0), 5.5, 0.2, 5.5, marmol_fondo.clone())
                .con_mosaico(1.6),
        ),
        // Una linea de oro sobre cada borde.
        bloque(0.0, 0.7, PISCINA, 5.5, 0.1, 0.3, &oro),
        bloque(0.0, 0.7, -PISCINA, 5.5, 0.1, 0.3, &oro),
        bloque(-PISCINA, 0.7, 0.0, 0.3, 0.1, 5.5, &oro),
        bloque(PISCINA, 0.7, 0.0, 0.3, 0.1, 5.5, &oro),
    ];

    // El agua: UN plano liso a y = 0.1, recortado al cuadrado interior de
    // los bordes. Un plano infinito a esa altura cubriria la cueva entera.
    // Sin oleaje: el agua de la fuente no se mueve.
    // Con oleaje: anillos que salen del pedestal y viajan hacia los
    // bordes. La fuerza la pone la cancion (el bajo) y la fase el tiempo,
    // en `animacion::actualizar_escena`; aca solo se deja armada.
    const AGUA_Y: f32 = 0.1;
    piscina.push(Box::new(Plane {
        point: Vec3::new(0.0, AGUA_Y, 0.0),
        normal: Vec3::new(0.0, 1.0, 0.0),
        ripple_center: Vec3::zeros(),
        ripple_strength: 0.08,
        ripple_scale: 3.0,
        ripple_phase: 0.0,
        // La red de causticas de `water_fairy.png` se repite cada 0.8
        // unidades (antes cada dos).
        //
        // La textura del agua ahora ES una red de causticas —filamentos
        // de luz irregulares, que es el patron que dibuja una superficie
        // ondulada al concentrar la luz— y una caustica se lee como
        // caustica por la DENSIDAD de la red. Estirada a dos unidades por
        // celda quedaban tres celdas en toda la piscina y el patron se
        // leia como manchas; a 0.8 entran siete y se lee como agua.
        uv_scale: 1.25,
        limite: Some(Limite::Rectangulo(PISCINA - 0.3, PISCINA - 0.3)),
        material: agua.clone(),
    }));
    escena.registrar_agua(objects.len(), piscina.len() - 1, agua.emission_color);
    objects.push(Box::new(GrupoAcotado::new(piscina)));

    // --- 3. LAS COLUMNAS ---
    // Seis, en circulo de radio 3.5 alrededor de la piscina. El angulo
    // arranca en cero sobre +X, o sea que las columnas caen en 0, 60, 120,
    // ... grados y ninguna queda justo al frente (+Z, 90 grados) tapando
    // la Triforce: las dos de adelante estan a 30 grados de cada lado.
    // Cada una es base cuadrada, fuste cilindrico y capitel, en su propio
    // grupo acotado.
    const COLUMNAS: usize = 6;
    const COLUMNAS_RADIO: f32 = 3.5;
    const COLUMNA_ALTURA: f32 = 4.5;
    for i in 0..COLUMNAS {
        let angulo = i as f32 * 2.0 * PI / COLUMNAS as f32;
        let (x, z) = (COLUMNAS_RADIO * angulo.cos(), COLUMNAS_RADIO * angulo.sin());

        let columna: Vec<Box<dyn RayIntersect + Send + Sync>> = vec![
            cubo(x, -0.15, z, 0.7, &marmol),
            Box::new(Cylinder {
                center: Vec3::new(x, 0.0, z),
                radius: 0.25,
                height: COLUMNA_ALTURA,
                inner_radius: 0.0,
                // Una vuelta de textura cada unidad: las vetas del marmol
                // se leen a la misma escala que en los bloques.
                tile_size: 1.0,
                material: marmol.clone(),
            }),
            cubo(x, COLUMNA_ALTURA, z, 0.6, &marmol),
        ];
        objects.push(Box::new(GrupoAcotado::new(columna)));
    }

    // --- 4. EL TECHO ---
    // Dos vigas en cruz sobre los capiteles, un marco alrededor con su
    // moldura de oro, y cuatro losas que dejan el CENTRO ABIERTO: por ahi
    // entra el cielo y la luz cenital cae sobre la Triforce.
    const TECHO: f32 = 3.5;
    let techo: Vec<Box<dyn RayIntersect + Send + Sync>> = vec![
        // Vigas.
        bloque(0.0, 5.0, 0.0, 8.0, 0.5, 0.8, &marmol),
        bloque(0.0, 5.0, 0.0, 0.8, 0.5, 8.0, &marmol),
        // Marco.
        bloque(0.0, 5.3, TECHO, 8.0, 0.3, 0.5, &marmol),
        bloque(0.0, 5.3, -TECHO, 8.0, 0.3, 0.5, &marmol),
        bloque(-TECHO, 5.3, 0.0, 0.5, 0.3, 7.0, &marmol),
        bloque(TECHO, 5.3, 0.0, 0.5, 0.3, 7.0, &marmol),
        // Moldura de oro por debajo del marco.
        bloque(0.0, 5.15, TECHO, 7.5, 0.1, 0.3, &oro),
        bloque(0.0, 5.15, -TECHO, 7.5, 0.1, 0.3, &oro),
        bloque(-TECHO, 5.15, 0.0, 0.3, 0.1, 7.0, &oro),
        bloque(TECHO, 5.15, 0.0, 0.3, 0.1, 7.0, &oro),
        // Losas, con el hueco de 3 x 3 en el medio.
        bloque(-2.5, 5.5, 0.0, 2.0, 0.2, 7.0, &marmol),
        bloque(2.5, 5.5, 0.0, 2.0, 0.2, 7.0, &marmol),
        bloque(0.0, 5.5, -2.5, 3.0, 0.2, 2.0, &marmol),
        bloque(0.0, 5.5, 2.5, 3.0, 0.2, 2.0, &marmol),
    ];
    objects.push(Box::new(GrupoAcotado::new(techo)));

    // --- 5. PEDESTAL Y TRIFORCE ---
    // En el centro del agua: dos cubos de obsidiana apilados (el de abajo
    // medio hundido, asi emerge del agua) y encima los tres triangulos de
    // oro de la Triforce, en el plano XY mirando a +Z, hacia la camara.
    // Los vertices van en sentido antihorario vistos desde +Z: el Triangle
    // saca la normal de ese orden y no la voltea. La camara nunca pasa
    // detras (la orbita se topa antes), asi que con una cara alcanza.
    let mut altar: Vec<Box<dyn RayIntersect + Send + Sync>> = vec![
        cubo(0.0, 0.1, 0.0, 0.9, &obsidiana),
        cubo(0.0, 0.85, 0.0, 0.62, &obsidiana),
    ];
    let triforce = |v0: (f32, f32), v1: (f32, f32), v2: (f32, f32)| {
        Box::new(Triangle {
            a: Vec3::new(v0.0, v0.1, 0.0),
            b: Vec3::new(v1.0, v1.1, 0.0),
            c: Vec3::new(v2.0, v2.1, 0.0),
            uv_a: None,
            uv_b: None,
            uv_c: None,
            material: oro_sagrado.clone(),
        }) as Box<dyn RayIntersect + Send + Sync>
    };
    // EL TAMANO IMPORTA MAS DE LO QUE PARECE. Estos tres triangulos median
    // 0.8 de ancho, que a once unidades de camara son treinta y cinco
    // pixeles del cuadro trazado: el HUECO del medio, que es la mitad de
    // eso, caia en diecisiete, y el bloom —que sobre el oro emisivo llega a
    // derramarse cincuenta— lo tapaba por completo. El resultado era que la
    // Triforce, que es el simbolo por el que se reconoce toda la escena, se
    // leia como un triangulo amarillo solido.
    //
    // A 1.36 de ancho el hueco mide sesenta pixeles en pantalla y sobrevive
    // al halo. Es el objeto que da sentido al resto: merece el tamano.
    const TRI: f32 = 0.68;
    const TRI_Y: f32 = 1.30;
    const TRI_ALTO: f32 = 0.68;
    altar.push(triforce(
        (-TRI, TRI_Y),
        (0.0, TRI_Y),
        (-TRI / 2.0, TRI_Y + TRI_ALTO),
    ));
    altar.push(triforce(
        (0.0, TRI_Y),
        (TRI, TRI_Y),
        (TRI / 2.0, TRI_Y + TRI_ALTO),
    ));
    altar.push(triforce(
        (-TRI / 2.0, TRI_Y + TRI_ALTO),
        (TRI / 2.0, TRI_Y + TRI_ALTO),
        (0.0, TRI_Y + TRI_ALTO * 2.0),
    ));
    // Los tres triangulos son los hijos 2, 3 y 4 del altar: la cancion
    // los hace latir.
    let emision_triforce = oro_sagrado.emission_color.unwrap_or(Color::WHITE);
    escena.registrar_triforce(
        objects.len(),
        (2..5).map(|i| (i, emision_triforce)).collect(),
    );
    objects.push(Box::new(GrupoAcotado::new(altar)));

    // --- 5a. LOS ANILLOS DE LA TRIFUERZA ---
    //
    // Tres toros finos rodeandola, inclinados y girando despacio (ver
    // `animacion::actualizar_anillos`). Son la unica figura de la escena
    // que no se resuelve con una cuadratica: la interseccion de un rayo con
    // un toro sale de una CUARTICA, y `toro.rs` la resuelve por Ferrari.
    //
    // Emisivos, o sea que no tapan la luz y quedan afuera del arbol de
    // sombras: tres anillos proyectando sombra sobre la Trifuerza la
    // taparian justo a ella, que es el objeto que le da sentido a la
    // escena. Lo que hacen es dibujar tres lineas de luz en el aire.
    //
    // EL GRUPO VA CON MARGEN, y el margen es el radio entero: los anillos
    // GIRAN y el arbol se queda con la caja del momento en que se armo (esa
    // leccion ya la dejaron las estelas). Girando, un anillo barre la
    // esfera de radio `ANILLO_RADIO + ANILLO_GROSOR`, y esa tiene que ser
    // su caja de nacimiento.
    let centro_anillos = Vec3::new(0.0, animacion::ANILLO_Y, 0.0);
    let anillos: Vec<Box<dyn RayIntersect + Send + Sync>> = (0..animacion::ANILLOS)
        .map(|_| {
            Box::new(Toro::nuevo(
                centro_anillos,
                Vec3::new(0.0, 1.0, 0.0),
                animacion::ANILLO_RADIO,
                animacion::ANILLO_GROSOR,
                Material::new(
                    [0.25, 0.85, 0.0, 0.0],
                    28.0,
                    0.0,
                    Texture::Solid(Color::new(210, 225, 255, 255)),
                    Some(Color::new(120, 140, 200, 255)),
                ),
            )) as Box<dyn RayIntersect + Send + Sync>
        })
        .collect();
    escena.registrar_anillos(objects.len());
    objects.push(Box::new(GrupoAcotado::con_margen(
        anillos,
        animacion::ANILLO_ALCANCE,
    )));

    // --- 5b. LOS CRISTALES ---
    // Cuatro racimos en las esquinas de la plaza: un cristal alto y dos
    // bajos, cada uno un prisma (cubo) con punta piramidal (cuatro
    // triangulos), y una luz de hada adentro del grande. La camara ve las
    // hadas y el agua REFRACTADAS a traves de ellos, y la luz de adentro
    // sale desenfocada por el bloom. Cada racimo lleva su propio color y
    // se enciende con una familia armonica distinta (ver
    // `animacion::energia_region`).
    //
    // Los triangulos de la punta se arman mirando hacia afuera: el
    // Triangle saca la normal del orden de los vertices y no la voltea,
    // asi que `cara` comprueba el lado y da vuelta el orden si hace falta.
    let cara = |a: Vec3, b: Vec3, c: Vec3, interior: Vec3, material: &Material| {
        let n = cross(&(b - a), &(c - a));
        let centro = (a + b + c) / 3.0;
        let (b, c) = if dot(&n, &(centro - interior)) < 0.0 { (c, b) } else { (b, c) };
        Box::new(Triangle {
            a,
            b,
            c,
            uv_a: Some((0.0, 0.0)),
            uv_b: Some((1.0, 0.0)),
            uv_c: Some((0.5, 1.0)),
            material: material.clone(),
        }) as Box<dyn RayIntersect + Send + Sync>
    };
    let cristal_en = |x: f32,
                      z: f32,
                      lado: f32,
                      alto: f32,
                      punta: f32,
                      con_luz: Option<Material>| {
        let base_y = -0.4;
        let mut piezas: Vec<Box<dyn RayIntersect + Send + Sync>> = vec![Box::new(
            Cube::new_rect(Vec3::new(x, base_y + alto / 2.0, z), lado, alto, lado, cristal.clone())
                .con_mosaico(1.5),
        )];
        let h = lado / 2.0;
        let top = base_y + alto;
        let apice = Vec3::new(x, top + punta, z);
        let interior = Vec3::new(x, top, z);
        let esquinas = [
            Vec3::new(x - h, top, z - h),
            Vec3::new(x + h, top, z - h),
            Vec3::new(x + h, top, z + h),
            Vec3::new(x - h, top, z + h),
        ];
        for i in 0..4 {
            piezas.push(cara(esquinas[i], esquinas[(i + 1) % 4], apice, interior, &cristal));
        }
        if let Some(material) = con_luz {
            piezas.push(Box::new(Sphere {
                center: Vec3::new(x, base_y + alto * 0.55, z),
                radius: lado * 0.22,
                material,
            }));
        }
        piezas
    };
    for (region, (sx, sz)) in [(1.0f32, 1.0f32), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)]
        .into_iter()
        .enumerate()
    {
        let (cx, cz) = (4.9 * sx, 4.9 * sz);
        let color = color_f(
            CRISTAL_COLORES[region].0,
            CRISTAL_COLORES[region].1,
            CRISTAL_COLORES[region].2,
        );
        let mut racimo = cristal_en(cx, cz, 0.55, 1.5, 0.7, Some(luz_de_cristal(CRISTAL_COLORES[region])));
        // La esfera es el ultimo hijo del cristal grande: un cubo, las
        // cuatro caras de la punta, y despues la luz.
        let luz = racimo.len() - 1;
        racimo.extend(cristal_en(cx - 0.5 * sx, cz + 0.15 * sz, 0.3, 0.8, 0.35, None));
        racimo.extend(cristal_en(cx + 0.1 * sx, cz - 0.55 * sz, 0.26, 0.6, 0.3, None));
        escena.registrar_cristal(objects.len(), luz, color, region);
        objects.push(Box::new(GrupoAcotado::new(racimo)));
    }

    // --- 5c. EL EMBLEMA EN EL MURO ---
    // La Trifuerza grabada en la estela del fondo, en grande y apenas
    // encendida: no es un objeto de la escena sino el MOTIVO del lugar,
    // como el escudo tallado sobre la puerta de un templo. Esta lejos y en
    // penumbra a proposito; lo que hace es contarte donde estas cuando la
    // camara se abre.
    //
    // La cara de la estela esta en z = -10.3, asi que el emblema va un
    // pelo delante para que no se pelee con ella por el mismo pixel.
    let emblema = |v0: (f32, f32), v1: (f32, f32), v2: (f32, f32)| {
        Box::new(Triangle {
            a: Vec3::new(v0.0, v0.1, -10.25),
            b: Vec3::new(v1.0, v1.1, -10.25),
            c: Vec3::new(v2.0, v2.1, -10.25),
            uv_a: None,
            uv_b: None,
            uv_c: None,
            material: oro_muro.clone(),
        }) as Box<dyn RayIntersect + Send + Sync>
    };
    // La estela va de y = -1 a y = 5 y mide 3.6 de ancho, asi que el
    // emblema entra entero con margen: base en 2.2, lado 1.0, punta en
    // 4.2. La altura tampoco es libre: mas abajo lo tapaban el altar y
    // las columnas del fondo (se probo, y del emblema se veia un solo
    // triangulo asomando), y mas arriba lo corta la losa del techo.
    let (ex, ey, ee) = (0.0f32, 2.2f32, 1.0f32);
    objects.push(Box::new(GrupoAcotado::new(vec![
        emblema((ex - ee, ey), (ex, ey), (ex - ee / 2.0, ey + ee)),
        emblema((ex, ey), (ex + ee, ey), (ex + ee / 2.0, ey + ee)),
        emblema((ex - ee / 2.0, ey + ee), (ex + ee / 2.0, ey + ee), (ex, ey + 2.0 * ee)),
    ])));

    // --- 5d. LAS RUPIAS ---
    // Tres, flotando y girando despacio sobre el agua. Cada una es una
    // BIPIRAMIDE HEXAGONAL: seis triangulos desde la punta de arriba al
    // hexagono del ecuador y seis desde el de abajo, que es exactamente la
    // forma de la rupia del juego. Doce caras planas es lo que hace que
    // gire destellando: cada una alcanza a la luz en un momento distinto.
    //
    // Refractan, asi que a traves de ellas se ve el agua deformada y con
    // su color. Van registradas para que la animacion las haga girar.
    let rupia_en = |centro: Vec3, alto: f32, radio: f32| -> Vec<(Vec3, Vec3, Vec3)> {
        let arriba = centro + Vec3::new(0.0, alto, 0.0);
        let abajo = centro - Vec3::new(0.0, alto, 0.0);
        let ecuador: Vec<Vec3> = (0..6)
            .map(|i| {
                let a = i as f32 * PI / 3.0;
                centro + Vec3::new(radio * a.cos(), 0.0, radio * a.sin())
            })
            .collect();

        let mut caras = Vec::with_capacity(12);
        for i in 0..6 {
            let (p, q) = (ecuador[i], ecuador[(i + 1) % 6]);
            // El orden deja la normal mirando hacia afuera en las dos
            // mitades: arriba antihorario visto desde afuera, abajo al
            // reves, porque la cara apunta para el otro lado.
            caras.push((p, q, arriba));
            caras.push((q, p, abajo));
        }
        caras
    };
    for (i, (x, z, y, tinte, emision)) in [
        (-1.9f32, 1.6f32, 1.5f32, color_f(0.3, 1.0, 0.45), color_f(0.15, 0.55, 0.2)),
        (2.0, -1.3, 1.9, color_f(0.35, 0.6, 1.0), color_f(0.15, 0.3, 0.6)),
        (0.4, 2.3, 2.6, color_f(1.0, 0.35, 0.35), color_f(0.5, 0.12, 0.12)),
    ]
    .into_iter()
    .enumerate()
    {
        let centro = Vec3::new(x, y, z);
        let caras = rupia_en(centro, 0.26, 0.15);
        let material = rupia(tinte, emision);
        let piezas: Vec<Box<dyn RayIntersect + Send + Sync>> = caras
            .iter()
            .map(|(a, b, c)| {
                Box::new(Triangle {
                    a: *a,
                    b: *b,
                    c: *c,
                    uv_a: None,
                    uv_b: None,
                    uv_c: None,
                    material: material.clone(),
                }) as Box<dyn RayIntersect + Send + Sync>
            })
            .collect();
        escena.registrar_rupia(objects.len(), centro, caras, i as f32 * 2.1);
        objects.push(Box::new(GrupoAcotado::con_margen(piezas, animacion::RUPIA_FLOTE)));
    }

    // --- 5e. LA ENTRADA: ESCALINATA Y ANTORCHAS ---
    // Dos peldaños que suben de la plaza al borde de la piscina, con su
    // linea de oro en el canto, y dos antorchas flanqueandolos: un pie de
    // obsidiana, un pebetero de oro y una llama. Las antorchas son la
    // unica fuente CALIDA de la escena, y lo unico que dice que alguien
    // estuvo aca y encendio algo.
    //
    // Van todas en UN grupo y no en tres. No es orden: es el costo de las
    // sombras. Cada rayo de sombra recorre la lista de los que pueden
    // tapar la luz, asi que cada entrada de esa lista se paga en TODOS los
    // impactos de TODAS las luces. Con las tres piezas sueltas, agregar la
    // entrada subia el cuadro de 38 a 45 milisegundos; metidas en un grupo
    // (que se descarta con una sola cuenta si el rayo pasa lejos), la
    // lista vuelve a medir lo que medía antes.
    let mut entrada: Vec<Box<dyn RayIntersect + Send + Sync>> = vec![
        bloque(0.0, -0.2, 4.0, 3.2, 0.4, 0.6, &marmol),
        bloque(0.0, -0.03, 3.5, 3.2, 0.75, 0.4, &marmol),
        bloque(0.0, 0.0, 4.3, 3.0, 0.06, 0.06, &oro),
        bloque(0.0, 0.35, 3.7, 3.0, 0.06, 0.06, &oro),
    ];
    for lado in [-1.0f32, 1.0] {
        let (x, z) = (lado * 1.9, 4.5);
        entrada.push(bloque(x, 0.15, z, 0.16, 1.3, 0.16, &obsidiana));
        entrada.push(bloque(x, 0.85, z, 0.30, 0.12, 0.30, &oro));
        entrada.push(Box::new(Sphere {
            center: Vec3::new(x, 0.98, z),
            radius: 0.13,
            material: fuego.clone(),
        }));
    }
    objects.push(Box::new(GrupoAcotado::new(entrada)));

    // --- 6. LAS HADAS ---
    // Veintitres esferitas emisivas flotando en circulos alrededor de la
    // Triforce, como en la fuente original: un anillo bajo y cercano de
    // rosas (adentro de la piscina, para que el agua las refleje), otro
    // mas alto y amplio con algunas cyan, cuatro chiquitas cerca del
    // techo, y tres doradas pegadas a la Triforce. Cada anillo va en su
    // grupo acotado, registrado para que el brillo siga los arpegios del
    // arpa.
    let rosa = color_f(1.2, 0.3, 0.7);
    let cyan = color_f(0.2, 0.8, 1.0);
    let dorado = color_f(1.0, 0.8, 0.2);

    // Un radio que varia entre `min` y `max` segun la fase `f` (en
    // radianes): los orbes de un anillo no son todos iguales.
    let radio_entre = |min: f32, max: f32, f: f32| min + (max - min) * (f.sin() * 0.5 + 0.5);

    let mut anillos: Vec<Vec<(Vec3, f32, Color)>> = Vec::new();

    // Anillo bajo, las principales: ocho rosas a 1.5 del centro, con la
    // altura ondulando entre 1.3 y 2.3.
    anillos.push(
        (0..8)
            .map(|i| {
                let angulo = i as f32 * 2.0 * PI / 8.0;
                let y = 1.8 + (i as f32 * 0.3).sin() * 0.5;
                (
                    Vec3::new(1.5 * angulo.cos(), y, 1.5 * angulo.sin()),
                    radio_entre(0.07, 0.10, i as f32 * 0.9),
                    rosa,
                )
            })
            .collect(),
    );

    // Anillo alto y amplio: ocho a 2.5 del centro, corridas 22.5 grados
    // respecto del anillo bajo, mas chicas porque estan mas lejos, y una
    // de cada tres cyan.
    anillos.push(
        (0..8)
            .map(|i| {
                let angulo = i as f32 * 2.0 * PI / 8.0 + PI / 8.0;
                let y = 3.0 + (i as f32 * 0.5).cos() * 0.4;
                (
                    Vec3::new(2.5 * angulo.cos(), y, 2.5 * angulo.sin()),
                    radio_entre(0.05, 0.08, i as f32 * 1.1),
                    if i % 3 == 0 { cyan } else { rosa },
                )
            })
            .collect(),
    );

    // Sueltas arriba, cerca del techo: cuatro puntitos rosas, y las tres
    // doradas alrededor de la Triforce.
    let mut altas: Vec<(Vec3, f32, Color)> = (0..4)
        .map(|i| {
            let angulo = i as f32 * 2.0 * PI / 4.0 + PI / 4.0;
            let y = 4.0 + (i as f32 * 0.7).sin() * 0.3;
            (
                Vec3::new(angulo.cos(), y, angulo.sin()),
                radio_entre(0.04, 0.06, i as f32 * 1.3),
                rosa,
            )
        })
        .collect();
    altas.extend([
        (Vec3::new(0.3, 2.3, 0.1), 0.05, dorado),
        (Vec3::new(-0.2, 2.0, -0.2), 0.04, dorado),
        (Vec3::new(0.0, 2.6, 0.15), 0.04, dorado),
    ]);
    anillos.push(altas);

    // Las hadas CAEN despacio y renacen arriba (ver `animacion::hada_en`),
    // asi que sus grupos se arman con margen: la esfera acotante tiene que
    // cubrir hasta donde pueden llegar, no solo donde nacen.
    for hadas in anillos {
        let bases: Vec<(Vec3, f32, Color)> = hadas.clone();
        let orbes: Vec<Box<dyn RayIntersect + Send + Sync>> = hadas
            .into_iter()
            .map(|(center, radius, emision)| {
                Box::new(Sphere {
                    center,
                    radius,
                    material: orbe(emision),
                }) as Box<dyn RayIntersect + Send + Sync>
            })
            .collect();
        escena.registrar_orbes(objects.len(), bases);
        objects.push(Box::new(GrupoAcotado::con_margen(orbes, animacion::HADA_ALCANCE)));
    }

    // --- 6b. LAS ESTELAS DEL ARPA ---
    //
    // Cada pocos ataques del arpa, una luz cruza la fuente pasando entre
    // las columnas. Es la idea de la estrella fugaz, pero traida ADENTRO
    // de la cueva: se probo primero en el cielo y no se veia, porque con
    // la camara mirando catorce grados hacia abajo y el letterbox
    // comiendose el borde de arriba, el cielo visible es una franja de
    // siete grados pegada al horizonte y encima tapada a trozos por las
    // losas del techo. Aca cruzan por donde la camara esta mirando.
    //
    // Cada estela son ocho esferitas emisivas en fila, la cabeza mas
    // grande y las de atras apagandose. Van TODAS en un grupo, con margen
    // de sobra: cruzan la escena entera y la caja acotante se calcula una
    // sola vez (ver `GrupoAcotado::con_margen`).
    let estela_mat = Material::new(
        [1.0, 0.0, 0.0, 0.0],
        1.0,
        0.0,
        Texture::Solid(color_f(1.0, 0.95, 1.0)),
        // Nace apagada: la animacion le escribe el brillo cuando le toca.
        Some(Color::new(0, 0, 0, 255)),
    );
    // UN GRUPO POR ESTELA, y no uno solo con las treinta esferas. La caja
    // de cada uno se RECALCULA en cada cuadro alrededor de donde quedo esa
    // estela (ver `GrupoAcotado::recalcular_caja`): con una sola caja fija
    // que cubriera los tres recorridos, cualquier rayo que entrara a la
    // fuente probaba las treinta esferas aunque no hubiera ninguna
    // encendida, y eso costaba siete milisegundos por cuadro.
    for _ in 0..animacion::ESTELAS {
        // La cabeza (esfera) y la cola (cilindro orientado). Nacen
        // invisibles; la animacion las enciende cuando les toca.
        let segmentos: Vec<Box<dyn RayIntersect + Send + Sync>> = vec![
            Box::new(Sphere {
                center: Vec3::new(0.0, 2.5, 0.0),
                radius: 0.0,
                material: estela_mat.clone(),
            }),
            Box::new(cylinder::CilindroOrientado::nuevo(
                Vec3::new(0.0, 2.5, 0.0),
                Vec3::new(0.0, 2.5, 0.1),
                0.0,
                0.0,
                estela_mat.clone(),
            )),
        ];
        escena.registrar_estela(objects.len());
        // CON MARGEN GRANDE, aunque despues la caja se recalcule cada
        // cuadro. El BVH se construye UNA vez y se queda con su propia
        // copia de la caja de cada objeto, asi que la que vale para podar
        // el arbol es la de este momento: si naciera chica (las esferas
        // arrancan con radio cero), el arbol descartaria la estela para
        // siempre y no se veria nunca por mas que despues se moviera.
        //
        // El recalculo de cada cuadro NO reemplaza a esto: afina la prueba
        // que el grupo se hace A SI MISMO cuando el rayo ya llego hasta
        // el, que es la que evita probar las diez esferas.
        objects.push(Box::new(GrupoAcotado::con_margen(
            segmentos,
            animacion::ESTELA_ALCANCE,
        )));
    }

    // --- 7. POLVO DE HADA ---
    // Sesenta esferas DIMINUTAS esparcidas por el volumen de la
    // fuente: particulas de polvo magico en el aire. Mas debiles que las
    // hadas, pero con el bloom se leen como puntitos de brillo. Las
    // posiciones salen de un hash del indice, asi el polvo es el mismo en
    // cada corrida. No se registran en la escena viva: son decoracion
    // constante, sin sync.
    //
    // Van en cuatro grupos, uno por cuadrante en x y z, para que un rayo
    // que pasa lejos descarte de a nueve con una sola cuenta.
    let polvo = Material::new(
        [1.0, 0.0, 0.0, 0.0],
        1.0,
        0.0,
        Texture::Solid(color_f(1.0, 0.9, 0.95)),
        Some(color_f(0.7, 0.45, 0.65)),
    );
    let mut cuadrantes: [Vec<Box<dyn RayIntersect + Send + Sync>>; 4] = Default::default();
    // Un hash multiplicativo DISTINTO por eje. Con uno solo y los otros
    // ejes sacados de multiplicarlo por 1.3 y 1.7, las motas quedaban
    // alineadas en hileras de puntitos (las tres coordenadas eran la misma
    // recta vista con tres escalas); con un multiplicador propio por eje
    // se desparraman.
    let hash = |i: u64, mult: u64| ((i.wrapping_mul(mult)) >> 16) as f32;
    for i in 0..60u64 {
        let x = (hash(i, 2_654_435_761) % 600.0) / 100.0 - 3.0;
        let y = (hash(i, 2_246_822_519) % 450.0) / 100.0 + 0.5;
        let z = (hash(i, 3_266_489_917) % 600.0) / 100.0 - 3.0;
        let radius = 0.015 + (hash(i, 668_265_263) % 100.0) / 100.0 * 0.01;

        let cuadrante = (x >= 0.0) as usize + 2 * (z >= 0.0) as usize;
        cuadrantes[cuadrante].push(Box::new(Sphere {
            center: Vec3::new(x, y, z),
            radius,
            material: polvo.clone(),
        }));
    }
    // Las motas DERIVAN (ver `animacion`), asi que sus grupos van con
    // margen y se registran con el centro de nacimiento de cada una.
    for motas in cuadrantes {
        if !motas.is_empty() {
            let centros: Vec<Vec3> = motas
                .iter()
                .filter_map(|m| m.bounds().map(|(c, _)| c))
                .collect();
            escena.registrar_polvo(objects.len(), centros);
            objects.push(Box::new(GrupoAcotado::con_margen(motas, animacion::POLVO_ALCANCE)));
        }
    }

    // ============================================================
    //  LUCES
    // ============================================================
    //
    // Cinco, y fuertes: la fuente es un lugar BRILLANTE. La cyan cenital
    // cae por el hueco del techo y le da el tono a toda la fuente; la rosa
    // vive en el centro, entre las hadas; las dos teal de los costados
    // levantan las columnas y el marmol; y la del agua ilumina las
    // columnas y el techo DESDE ABAJO, que es lo que pasa en la fuente
    // original: el agua es la que alumbra.
    //
    // El orden IMPORTA: es el mismo que usa `light_multipliers` en
    // `sync.rs`. Si se agrega o se saca una luz hay que tocar las dos.
    let mut lights = vec![
        // 0. Violeta cenital, sobre el hueco del techo. Era cyan, ahora
        //    violeta para que la luz que baja del cielo tina la escena de
        //    morado en vez de ahogarla en cyan.
        Light::new(Vec3::new(0.0, 8.0, 0.0), color_f(0.72, 0.5, 0.95), 1.4),
        // 1. Rosa en el centro, donde estan las hadas. LA MAS FUERTE: el
        //    rosa es el color principal de la Fairy Fountain.
        Light::new(Vec3::new(0.0, 3.0, 0.0), color_f(1.0, 0.55, 0.75), 1.6),
        // 2 y 3. Una lavanda y una teal, para que los lados no sean iguales.
        Light::new(Vec3::new(-5.0, 3.0, -3.0), color_f(0.6, 0.3, 0.7), 0.9),
        Light::new(Vec3::new(5.0, 3.0, -3.0), color_f(0.3, 0.7, 0.8), 0.9),
        // 4. La luz del agua, desde abajo. Justo encima del pedestal y no
        //    a ras del agua (y = 0.5): ahi quedaba en la ranura entre los
        //    dos cubos de obsidiana, que la tapaban casi entera. En 1.2
        //    esta por encima del cubo de arriba y por debajo de la
        //    Triforce, que es emisiva y no tapa. El cyan queda ACA, que es
        //    donde corresponde: el agua es la fuente del azul.
        Light::new(Vec3::new(0.0, 1.2, 0.0), color_f(0.2, 0.7, 0.8), 1.2),
        // 5. LA LUNA. Muy lejos, en la direccion en la que se la ve en el
        //    cielo, con alcance enorme para que llegue pareja a toda la
        //    escena: una luz fria y suave que viene de atras y por encima
        //    del borde de la cueva, y proyecta las sombras largas de las
        //    columnas hacia la camara. La cancion no la toca (el analisis
        //    solo maneja las cinco primeras).
        Light::new(
            vec3::normalize(&cielo::LUNA_DIRECCION) * 40.0,
            color_f(0.75, 0.8, 1.0),
            0.4,
        )
        .con_alcance(60.0),
        // 6 y 7. LAS ANTORCHAS. Naranjas, chicas y de ALCANCE CORTO (4
        //    unidades): alumbran su rincon de la escalinata y nada mas.
        //    Eso no es solo estetica, es lo que las hace baratas: con el
        //    corte por aporte de `cast_ray`, un impacto lejos de ellas ni
        //    siquiera les tira el rayo de sombra.
        Light::new(Vec3::new(-1.9, 0.98, 4.5), color_f(1.0, 0.6, 0.25), 2.4)
            .con_alcance(2.0),
        Light::new(Vec3::new(1.9, 0.98, 4.5), color_f(1.0, 0.6, 0.25), 2.4)
            .con_alcance(2.0),
    ];

    // Las luces se atenuan con la distancia (ver `Light::atenuacion`), y
    // las intensidades de arriba estaban afinadas sin eso: se compensan
    // todas parejo para que la fuente quede igual de brillante en el
    // centro y se apague hacia afuera.
    for luz in lights.iter_mut().take(5) {
        luz.intensity *= 1.15;
    }

    escena.registrar_luces(&lights);

    // El cielo: se genera una vez y se muestrea por cada rayo que no pega
    // en nada. `mut` porque cada cuadro se lo gira y se le sube el
    // amanecer (ver `avanzar_noche`).
    let mut cielo = Cielo::generar();

    // Quien puede tapar la luz. Se pregunta UNA vez, aca, y el bucle de
    // sombras recorre solo estos en vez de la lista entera. En este
    // escenario la diferencia es casi todo: los dos anillos y los laseres
    // son emisivos y quedan afuera, junto con las nueve esferas acotantes
    // que los envuelven.
    let occluders: Vec<usize> = objects
        .iter()
        .enumerate()
        .filter(|(_, o)| o.puede_tapar())
        .map(|(i, _)| i)
        .collect();

    // Los dos arboles. Se arman UNA vez, aca, y valen toda la corrida:
    // lo que se mueve durante la cancion (las hadas, las rupias, el agua)
    // vive adentro de grupos que se armaron con margen, asi que sus cajas
    // ya contemplan hasta donde puede llegar cada pieza.
    //
    // Va uno aparte para las sombras y no se reusa el grande porque la
    // mitad de los objetos son emisivos (las hadas, el polvo, las rupias,
    // el emblema) y no tapan a nadie: un arbol solo con los que si tapan
    // es mas chico y ademas descarta mejor, porque sus cajas no tienen que
    // cubrir cosas que igual se iban a ignorar.
    let (arbol, arbol_sombras) = arboles_de(&objects, &occluders);

    println!(
        "objetos: {} en total, {} pueden dar sombra",
        objects.len(),
        occluders.len()
    );

    // ============================================================
    //  AUDIO Y LINEA DE TIEMPO
    // ============================================================
    //
    // El dispositivo va ANTES que el reloj: el stream se lo presta, y Rust
    // suelta las variables al revez de como se declaran. Al reves, el
    // dispositivo se cerraria con el stream todavia abierto.
    let audio = RaylibAudio::init_audio_device()
        .map_err(|e| eprintln!("sin dispositivo de audio: {e:?}"))
        .ok();

    let mut reloj = RelojEscena::nuevo(audio.as_ref(), &AUDIO_PATHS);

    // El analisis de la cancion. No hay keyframes: todo lo que se mueve sale
    // de aca, y esto sale de correr `analizar_audio.py` sobre el mp3. Si el
    // JSON no aparece, `cargar` avisa y devuelve un analisis vacio con el
    // que la escena se ve quieta pero entera.
    let analisis = sync::SyncData::cargar(&sync::RUTAS_SYNC);

    if let Some(largo) = reloj.duracion() {
        // Un aviso, no un error: el JSON de otra cancion igual se puede
        // usar, solo que la escena va a ir por un lado y el audio por otro.
        if analisis.hay_analisis() && (largo - analisis.duracion).abs() > 1.0 {
            eprintln!(
                "sync: OJO, el mp3 dura {largo:.1} s y el analisis {:.1} s.                  Parecen temas distintos: volve a correr analizar_audio.py.",
                analisis.duracion
            );
        }
    }

    // ============================================================
    //  CAMARA
    // ============================================================
    //
    // Orbital, tipo pendulo: arranca de frente y elevada, cerca de la boca
    // de la cueva, mirando hacia adentro. Los cristales, el agua y la
    // Triforce quedan en cuadro. Ver `Orbita`.
    let mut orbita = Orbita::inicial();

    // Cuanto mueven las teclas por cuadro.
    const GIRO: f32 = 0.03;
    const INCLINACION: f32 = 0.02;
    const ZOOM: f32 = 0.3;

    // ============================================================
    //  MODO BANCO DE PRUEBAS  (--bench)
    // ============================================================
    //
    // Traza unos cuadros sueltos, informa cuanto cuesta cada resolucion de
    // la escalera y vuelca un PNG. No abre ventana: por eso `raylib::init`
    // esta mas abajo y no arriba de todo.
    //
    // Sirve para dos cosas que de otro modo no se pueden hacer: medir sin
    // que el vsync y el dibujado ensucien el numero, y mirar el resultado en
    // una maquina donde no haya pantalla disponible.
    // ============================================================
    //  MODO DIAGNOSTICO DE SINCRONIZACION  (--sync)
    // ============================================================
    //
    // Imprime lo que el analisis le pide a la escena, segundo a segundo, sin
    // trazar un solo rayo. Es la unica forma de contestar "por que no se ven
    // los laseres" sin mirar cuadros sueltos y adivinar: un cuadro es una
    // foto, y el chase es un fenomeno que solo existe en el tiempo.
    if std::env::args().any(|a| a == "--sync") {
        let paso: f32 = std::env::args()
            .skip_while(|a| a != "--sync")
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(2.0);

        println!(
            "\n{:>7}  {:>5} {:>5} {:>5} {:>5}  {:>3}  {:>5} {:>5} {:>5}  {:^12} {:^4}  haces",
            "t", "bass", "mid", "high", "total", "on", "bloom", "hadas", "swell", "do..si", "crist"
        );

        let mut t = 0.0f32;
        while t < analisis.duracion {
            let p = analisis.get_scene_params(t);
            let f = analisis.frame_publico(t);

            // Cuantos haces estan encendidos AHORA, y con que fuerza. La
            // barra es lo que se veria en pantalla.
            let barra: String = p
                .laser_emissions
                .iter()
                .map(|e| match e {
                    e if *e > 0.8 => '#',
                    e if *e > 0.3 => '+',
                    e if *e > 0.02 => '.',
                    _ => ' ',
                })
                .collect();

            // Las doce notas, para ver la MELODIA pasar. Cada columna es
            // una nota de do a si y cada hada se queda con una (ver
            // `animacion::nota_de`), asi que esta tira es, literalmente,
            // que hadas estan encendidas.
            let notas: String = p
                .notas
                .iter()
                .map(|n| match n {
                    n if *n > 0.55 => '#',
                    n if *n > 0.30 => '+',
                    n if *n > 0.12 => '.',
                    _ => ' ',
                })
                .collect();

            // Y los cuatro cristales: cada uno con su tercio del circulo
            // de quintas. Si se encendieran los cuatro a la vez, el
            // reparto no estaria diciendo nada.
            let cristales: String = (0..4)
                .map(|r| {
                    let e = p.armonia[r];
                    match e {
                        e if e > 0.55 => '#',
                        e if e > 0.30 => '+',
                        e if e > 0.12 => '.',
                        _ => ' ',
                    }
                })
                .collect();

            println!(
                "{t:7.1}  {:5.2} {:5.2} {:5.2} {:5.2}  {:>3}  {:5.2} {:5.2} {:5.2}  [{notas}] [{cristales}]  [{barra}]",
                f.bass,
                f.mid,
                f.high,
                f.total,
                if f.onset { "SI" } else { "" },
                p.bloom_strength,
                p.orb_emission,
                p.swell
            );

            t += paso;
        }

        // Cuanto tiempo hay AL MENOS un haz encendido. Es el numero que
        // decide si el rig se lee como encendido o como titilando.
        let mut prendidos = 0.0f64;
        let mut total = 0.0f64;
        let mut t = 0.0f32;
        // Y de paso, EL RITMO DE LAS ESTELAS. Va en el mismo barrido porque
        // es la otra cosa que un cuadro no puede contestar: una estela es un
        // evento, y en una foto o esta cruzando o no esta.
        let mut salidas: Vec<(usize, f32)> = Vec::new();
        let mut fugaces: Vec<(usize, f32)> = Vec::new();

        while t < analisis.duracion {
            let p = analisis.get_scene_params(t);
            if p.laser_emissions.iter().any(|e| *e > 0.02) {
                prendidos += 1.0;
            }
            for &salida in &p.estelas {
                if !salidas.contains(&salida) {
                    salidas.push(salida);
                }
            }
            for &salida in &p.estrellas {
                if !fugaces.contains(&salida) {
                    fugaces.push(salida);
                }
            }
            total += 1.0;
            t += 1.0 / 60.0;
        }
        println!(
            "\nhay al menos un haz encendido el {:.1}% del tiempo",
            prendidos / total * 100.0
        );

        // El ritmo de las dos cosas que la cancion lanza: las estelas del
        // arpa adentro de la cueva y las estrellas fugaces en el cielo.
        let ritmo = |que: &str, mut cuando: Vec<(usize, f32)>| {
            cuando.sort_by(|a, b| a.1.total_cmp(&b.1));
            let huecos: Vec<f32> = cuando.windows(2).map(|w| w[1].1 - w[0].1).collect();
            let mayor = huecos.iter().copied().fold(0.0f32, f32::max);
            let medio = huecos.iter().sum::<f32>() / huecos.len().max(1) as f32;
            println!(
                "{} {que}, una cada {medio:.1} s de media, hueco mas largo {mayor:.1} s",
                cuando.len()
            );
            // El reparto a lo largo del tema, de veinte en veinte segundos:
            // es donde se ve si el ritmo tiene arco o es un metronomo.
            print!("   reparto cada 20 s:");
            let mut desde = 0.0f32;
            while desde < analisis.duracion {
                let cuantas = cuando
                    .iter()
                    .filter(|(_, t)| (desde..desde + 20.0).contains(t))
                    .count();
                print!(" {cuantas}");
                desde += 20.0;
            }
            println!();
        };
        ritmo("estelas del arpa", salidas);
        ritmo("estrellas fugaces", fugaces);
        return;
    }

    // ============================================================
    //  BANCO DE PRUEBAS DEL ANTIALIASING TEMPORAL  (--taa)
    // ============================================================
    //
    // Simula la camara REAL en movimiento (el pendulo, a 20 cuadros por
    // segundo) acumulando como en vivo, y vuelca tres PNG del mismo
    // instante: el cuadro crudo de un rayo por pixel, el acumulado, y la
    // referencia con antialiasing de 2x2 rayos.
    //
    // Es la unica forma de comprobar que la reproyeccion hace lo que dice.
    // Un antialiasing temporal roto se ve igual de bien en una foto con la
    // camara quieta y emborrona todo en cuanto algo se mueve; comparando
    // los tres cuadros del MISMO instante con la camara andando, la
    // diferencia se mide en vez de discutirse.
    if std::env::args().any(|a| a == "--taa") {
        let dir = std::env::var("BENCH_OUT").unwrap_or_else(|_| ".".into());
        let (w, h) = (RENDER_W as usize, RENDER_H as usize);
        const CUADROS: u32 = 24;
        const DT: f32 = 1.0 / 20.0;
        let t0 = 87.0f32;

        let mut fb = Framebuffer::new(w, h, BACKGROUND);
        let mut orbita = Orbita::inicial();
        orbita.avanzar(t0, 0.0, 0.0, 0.0);
        let mut previa: Option<Camera> = None;
        let mut movimiento = 0.0f32;
        let mut costo_acum = 0.0f64;
        let mut cam = orbita.camara(2.0);
        let mut ambiente_ultimo = ambiente_de(0.0, 0.0);
        let mut fase_agua_ultima = 0.0f32;

        for n in 0..CUADROS {
            let t = t0 + n as f32 * DT;
            let params = analisis.get_scene_params(t);
            animacion::actualizar_escena(&mut objects, &mut lights, &escena, &params);
            avanzar_noche(&mut cielo, &mut lights, &params);
            cam = orbita.camara_cine(params.camera_target_y, params.cine);

            let jitter = (halton(n + 1, 2), halton(n + 1, 3));
            // La referencia de mas abajo se traza fuera de este bucle y
            // tiene que usar EXACTAMENTE la misma luz, o la comparacion
            // mediria el cambio de hora en vez del antialiasing.
            ambiente_ultimo = ambiente_de(params.luz_del_dia, params.swell);
            fase_agua_ultima = params.tiempo * animacion::AGUA_VELOCIDAD;
            fb.clear();
            render_rows(
                &mut fb, &objects, &cielo, &arbol, &arbol_sombras, &lights, &cam, MAX_DEPTH,
                false, jitter, ambiente_ultimo, fase_agua_ultima, n, 0, h,
            );

            if n + 1 == CUADROS {
                let _ = image::RgbaImage::from_raw(w as u32, h as u32, fb.to_rgba_opaco())
                    .map(|img| img.save(format!("{dir}/taa_crudo.png")));
            }

            if let Some(p) = previa.as_ref() {
                movimiento = movimiento_en_pixeles(p, &cam);
            }
            let t_acum = std::time::Instant::now();
            fb.acumular(TAA_PESO_MINIMO, &cam, previa.as_ref(), jitter);
            costo_acum += t_acum.elapsed().as_secs_f64() * 1000.0;
            previa = Some(cam);
            orbita.avanzar(DT, 0.0, 0.0, 0.0);
        }

        let _ = image::RgbaImage::from_raw(w as u32, h as u32, fb.to_rgba_opaco())
            .map(|img| img.save(format!("{dir}/taa_acumulado.png")));

        let mut ref_fb = Framebuffer::new(w, h, BACKGROUND);
        render_rows(
            &mut ref_fb, &objects, &cielo, &arbol, &arbol_sombras, &lights, &cam, MAX_DEPTH,
            true, (0.5, 0.5), ambiente_ultimo, fase_agua_ultima, 0, 0, h,
        );
        let _ = image::RgbaImage::from_raw(w as u32, h as u32, ref_fb.to_rgba_opaco())
            .map(|img| img.save(format!("{dir}/taa_referencia.png")));

        println!("la camara mueve la imagen {movimiento:.1} pixeles por cuadro a 20 fps");
        println!("acumular: {:.2} ms por cuadro", costo_acum / CUADROS as f64);
        println!("escritos taa_crudo.png, taa_acumulado.png y taa_referencia.png en {dir}");
        return;
    }

    if std::env::args().any(|a| a == "--bench") {
        let dir = std::env::var("BENCH_OUT").unwrap_or_else(|_| ".".into());

        // Solo se mide el TRAZADO. El post-procesado ya no esta en la CPU:
        // son cinco pasadas de shader sobre menos de un millon de pixeles,
        // o sea decimas de milisegundo que ademas no se pueden cronometrar
        // desde aca sin una ventana abierta.
        let (w, h) = (RENDER_W as usize, RENDER_H as usize);
        let (mut trazado, mut n) = (0.0, 0.0);
        let mut bandas = 0.0;

        // Los cinco momentos que valen la pena medir, y son los de la
        // estructura MEDIDA: el respiro inicial, el pico de la primera
        // seccion, la seccion plena del medio, el fondo del tema y el coro
        // final, que es el cuadro mas caro que existe.
        for &t in &[9.0f32, 30.0, 87.0, 160.0, 202.0] {
            let params = analisis.get_scene_params(t);
            // El pendulo en el mismo instante de la cancion.
            let mut cam_orbita = Orbita::inicial();
            cam_orbita.avanzar(t, 0.0, 0.0, 0.0);
            let cam = cam_orbita.camara_cine(params.camera_target_y, params.cine);
            animacion::actualizar_escena(&mut objects, &mut lights, &escena, &params);
            avanzar_noche(&mut cielo, &mut lights, &params);

            let mut fb = Framebuffer::new(w, h, BACKGROUND);
            let t0 = std::time::Instant::now();
            render_rows(
                &mut fb, &objects, &cielo, &arbol, &arbol_sombras, &lights, &cam, MAX_DEPTH,
                false, (0.5, 0.5), ambiente_de(params.luz_del_dia, params.swell),
                params.tiempo * animacion::AGUA_VELOCIDAD, 0, 0, h,
            );
            let ms = t0.elapsed().as_secs_f64() * 1000.0;

            // Y lo mismo pero POR BANDAS, que es como lo hace la escena en
            // vivo (el trazado se corta cada `FILAS_POR_BANDA` filas para
            // volver a rellenar el buffer de audio). Medir las dos es la
            // unica forma de saber si el corte se paga: son ocho despachos
            // a los hilos en vez de uno, y cada uno tiene que repartir el
            // trabajo y esperar a que el ultimo termine.
            fb.clear();
            let t1 = std::time::Instant::now();
            render(
                &mut fb, &objects, &cielo, &arbol, &arbol_sombras, &lights, &cam, MAX_DEPTH,
                false, (0.5, 0.5), ambiente_de(params.luz_del_dia, params.swell),
                params.tiempo * animacion::AGUA_VELOCIDAD, 0, || {},
            );
            let ms_bandas = t1.elapsed().as_secs_f64() * 1000.0;
            bandas += ms_bandas;

            trazado += ms;
            n += 1.0;
            println!("  t={t:5.0}s: {ms:6.1} ms de una  |  {ms_bandas:6.1} ms por bandas");

            // Un PNG por momento y no uno solo: los cinco cuadros juntos son
            // la unica forma de ver de un vistazo si la escena esta
            // siguiendo la cancion (el fondo del tema tiene que verse casi
            // vacio y el coro final reventado) o si se quedo clavada en un
            // estado intermedio.
            //
            // Salen CRUDOS, sin bloom ni niebla ni vinieta: eso lo pone la
            // GPU y aca no hay ventana. Es el cuadro que le llega al
            // pipeline, no el que se ve en pantalla.
            let _ = image::RgbaImage::from_raw(w as u32, h as u32, fb.to_rgba_opaco())
                .map(|img| img.save(format!("{dir}/{w}x{h}_t{t:.0}.png")));
        }

        let total = trazado / n;
        let total_bandas = bandas / n;
        println!("\n{w}x{h}: trazado {total:6.1} ms de media -> {:5.1} fps", 1000.0 / total);
        println!(
            "         por bandas de {FILAS_POR_BANDA} filas: {total_bandas:6.1} ms ({:+.1} ms)",
            total_bandas - total
        );
        return;
    }

    // ============================================================
    //  MODO FOTO  (--foto <segundos>)
    // ============================================================
    //
    // Abre la ventana, deja la escena en ese segundo de la cancion, traza
    // UN cuadro con toda la cadena de post-procesado (bloom, niebla, god
    // rays, caleidoscopio, grano) y guarda la pantalla en un PNG. Es la
    // unica forma de ver el cuadro FINAL sin grabar la pantalla: el
    // `--bench` vuelca el trazado crudo, que es la entrada de la GPU y no
    // lo que se ve.
    let foto: Option<f32> = std::env::args()
        .skip_while(|a| a != "--foto")
        .nth(1)
        .and_then(|s| s.parse().ok());

    // ============================================================
    //  VENTANA, BUFFERS Y TEXTURAS
    // ============================================================
    let (mut rl, thread) = raylib::init()
        .size(WIDTH as i32, HEIGHT as i32)
        .title("Cueva de las hadas - trazada con rayos")
        .build();

    // El buffer del trazador. Se crea UNA vez y se reusa toda la corrida:
    // el tamano no cambia nunca.
    let mut framebuffer =
        Framebuffer::new(RENDER_W as usize, RENDER_H as usize, BACKGROUND);

    // La textura por la que el cuadro trazado entra a la GPU. Tambien se
    // crea una sola vez; cada cuadro solo se le suben bytes con
    // `update_texture`. RGB es el color y ALPHA es la profundidad, que es lo
    // que la niebla del shader necesita.
    let mut texture = rl
        .load_texture_from_image(&thread, &framebuffer.to_image())
        .expect("no se pudo crear la textura");
    // Se dibuja estirada hasta la ventana; sin filtro bilineal raylib la
    // agranda repitiendo el pixel mas cercano y se ve en cuadrados con borde
    // duro. Con el, los 600 x 450 trazados llegan a 800 x 600 interpolados.
    texture.set_texture_filter(&thread, TextureFilter::TEXTURE_FILTER_BILINEAR);

    // Los shaders y los buffers intermedios del post-procesado. Se arman UNA
    // vez: compilar un shader y reservar un framebuffer cuestan
    // milisegundos.
    let mut post = PostGpu::nuevo(&mut rl, &thread);

    let mut antialias = false;

    // Promedio corrido del costo del cuadro, en segundos, solo para
    // mostrarlo. Corrido y no el ultimo valor: un solo cuadro lento (el
    // sistema hipando, el disco) haria saltar el numero sin que signifique
    // nada.
    let mut cuadro_medio = 0.033f32;

    println!("\ncontroles:");
    println!("  ESPACIO  play / pausa");
    println!("  izq/der o A/D    girar alrededor de la cueva");
    println!("  arriba/abajo o W/S   subir y bajar la camara");
    println!("  Q/E o PgUp/PgDn  acercar y alejar");
    println!("  mouse: arrastrar para orbitar, rueda para acercar");
    println!("  X        antialiasing 2x2 (cuadruplica el costo)");
    println!("  T        antialiasing temporal (gratis; prendido)");
    println!("  H        mostrar / ocultar la ayuda en pantalla");
    println!("  F        guardar una foto (PNG) del cuadro en pantalla");
    println!("  trazado fijo a {RENDER_W}x{RENDER_H}, estirado a {WIDTH}x{HEIGHT}");
    println!("  la camara se balancea sola todo el tiempo; las teclas se suman al balanceo\n");

    let mut anterior = std::time::Instant::now();
    let mut aviso_cuadro = true;

    // Lo que necesita el antialiasing temporal: la camara del cuadro
    // pasado (para saber cuanto se movio la imagen) y el numero de cuadro
    // (para correr el jitter con Halton).
    let mut camara_anterior: Option<Camera> = None;
    let mut cuadro_taa: u32 = 0;
    let mut taa = !std::env::args().any(|a| a == "--sin-taa");

    // La ayuda en pantalla: se ve los primeros segundos y despues se va
    // sola; H la trae de vuelta.
    let mut ayuda = true;
    let arranque = std::time::Instant::now();
    let mut cuadros: u32 = 0;

    // En modo foto la orbita se adelanta al mismo instante de la cancion,
    // como hace el bench, y la musica se deja en pausa.
    if let Some(t) = foto {
        orbita.avanzar(t, 0.0, 0.0, 0.0);
        if reloj.corriendo() {
            reloj.alternar_pausa();
        }
    }

    while !rl.window_should_close() {
        // Rellenar el buffer de audio. Va en CADA vuelta: si se saltea, el
        // sonido se corta apenas se vacia lo que raylib tenia por delante.
        reloj.actualizar();

        let ahora = std::time::Instant::now();
        // El tope evita que un cuadro larguisimo (el primero, o el sistema
        // hipando) le pegue un salto enorme a la orbita.
        let dt = (ahora - anterior).as_secs_f32().min(0.25);
        anterior = ahora;

        // EL numero del que depende toda la escena.
        let tiempo = foto.unwrap_or_else(|| reloj.tiempo());
        // Las tres capas ya vienen combinadas: estructura, ritmo y strobe.
        let params = analisis.get_scene_params(tiempo);

        // ---------- CONTROLES ----------
        if rl.is_key_pressed(KeyboardKey::KEY_SPACE) {
            reloj.alternar_pausa();
            println!(
                "{}  ({:.1} s)",
                if reloj.corriendo() { "play" } else { "pausa" },
                tiempo
            );
        }

        // X y no A: la A quedo para girar la camara.
        if rl.is_key_pressed(KeyboardKey::KEY_X) {
            antialias = !antialias;
            aviso_cuadro = true;
            camara_anterior = None;
            println!("antialiasing: {}", if antialias { "2x2" } else { "off" });
        }

        if rl.is_key_pressed(KeyboardKey::KEY_H) {
            ayuda = !ayuda;
        }

        if rl.is_key_pressed(KeyboardKey::KEY_T) {
            taa = !taa;
            camara_anterior = None;
            println!("antialiasing temporal: {}", if taa { "on" } else { "off" });
        }

        // Una foto del cuadro que esta en pantalla (el ANTERIOR: la captura
        // lee el framebuffer de la ventana, que es lo ultimo que se dibujo).
        //
        // En modo foto se esperan `FOTO_CUADROS` vueltas antes de disparar:
        // con la camara quieta, cada vuelta agrega una muestra al
        // acumulador temporal, asi que la foto sale con el antialiasing ya
        // convergido en vez del primer cuadro crudo.
        let guardar_foto = rl.is_key_pressed(KeyboardKey::KEY_F)
            || (foto.is_some() && cuadros == FOTO_CUADROS);

        // ---------- CAMARA ----------
        // El pendulo siempre corre; las teclas se le suman.
        let tecla = |a: KeyboardKey, b: KeyboardKey| rl.is_key_down(a) || rl.is_key_down(b);

        let mut d_theta = 0.0;
        let mut d_phi = 0.0;
        let mut d_radio = 0.0;

        if tecla(KeyboardKey::KEY_LEFT, KeyboardKey::KEY_A) {
            d_theta -= GIRO;
        }
        if tecla(KeyboardKey::KEY_RIGHT, KeyboardKey::KEY_D) {
            d_theta += GIRO;
        }
        if tecla(KeyboardKey::KEY_UP, KeyboardKey::KEY_W) {
            d_phi += INCLINACION;
        }
        if tecla(KeyboardKey::KEY_DOWN, KeyboardKey::KEY_S) {
            d_phi -= INCLINACION;
        }
        if tecla(KeyboardKey::KEY_Q, KeyboardKey::KEY_PAGE_UP) {
            d_radio -= ZOOM;
        }
        if tecla(KeyboardKey::KEY_E, KeyboardKey::KEY_PAGE_DOWN) {
            d_radio += ZOOM;
        }

        // El mouse: arrastrar con el boton izquierdo orbita (a la
        // derecha gira a la derecha, hacia arriba sube) y la rueda acerca.
        // Se suman a las teclas y al pendulo, con la misma tapa.
        if rl.is_mouse_button_down(MouseButton::MOUSE_BUTTON_LEFT) {
            let arrastre = rl.get_mouse_delta();
            d_theta += arrastre.x * 0.006;
            d_phi += arrastre.y * 0.004;
        }
        d_radio -= rl.get_mouse_wheel_move() * 0.4;

        // En modo foto la camara se queda quieta: el pendulo ya se
        // adelanto al instante pedido.
        if foto.is_some() {
            d_theta = 0.0;
            d_phi = 0.0;
            d_radio = 0.0;
        }
        orbita.avanzar(if foto.is_some() { 0.0 } else { dt }, d_theta, d_phi, d_radio);
        // Siempre apuntando al centro de la cueva, a la ALTURA que pide la
        // seccion de la cancion.
        let camera = orbita.camara_cine(params.camera_target_y, params.cine);

        // ---------- LA ESCENA SE MUEVE ----------
        // Antes de trazar, no despues: el cuadro que se dibuja abajo tiene
        // que ser el de este instante de la cancion.
        animacion::actualizar_escena(&mut objects, &mut lights, &escena, &params);
        avanzar_noche(&mut cielo, &mut lights, &params);

        // ---------- TRAZADO ----------
        let empezo = std::time::Instant::now();

        // El `reloj.actualizar()` de adentro es lo que mantiene el sonido
        // vivo mientras se traza: si el hilo se ausenta mas de lo que hay
        // cargado por delante, la musica se corta.
        //
        // El jitter recorre Halton, asi que las muestras de los ultimos
        // cuadros cubren el pixel parejo. Con el acumulador apagado va
        // clavado en el centro, que es lo de siempre.
        let jitter = if taa && !antialias {
            (halton(cuadro_taa + 1, 2), halton(cuadro_taa + 1, 3))
        } else {
            (0.5, 0.5)
        };
        cuadro_taa = cuadro_taa.wrapping_add(1);

        let t_trazado = std::time::Instant::now();
        framebuffer.clear();
        render(
            &mut framebuffer,
            &objects,
            &cielo,
            &arbol,
            &arbol_sombras,
            &lights,
            &camera,
            MAX_DEPTH,
            antialias,
            jitter,
            ambiente_de(params.luz_del_dia, params.swell),
            params.tiempo * animacion::AGUA_VELOCIDAD,
            cuadro_taa,
            || reloj.actualizar(),
        );
        let ms_trazado = t_trazado.elapsed().as_secs_f32() * 1000.0;

        // El acumulador temporal. El peso del cuadro nuevo sube con lo que
        // se movio la camara: quieta, un cuarto (converge sobre cuatro
        // muestras); movida mas de `TAA_MOVIMIENTO_MAXIMO` pixeles, uno
        // entero, o sea que la historia se tira y no hay arrastre.
        //
        // La reproyeccion se encarga del movimiento normal de la camara,
        // asi que el peso puede quedarse en el minimo casi siempre. Lo que
        // no cubre es un SALTO grande (el mouse arrastrando fuerte, una
        // tecla mantenida): ahi lo que se reproyecta ya es medio cuadro
        // distinto y el recorte de vecindad tiene que tirar tanto que no
        // vale la pena. Por eso el peso igual sube con el movimiento, solo
        // que con un umbral mucho mas alto que sin reproyectar.
        let peso = if !taa || antialias {
            1.0
        } else {
            match camara_anterior {
                None => 1.0,
                Some(previa) => {
                    let px = movimiento_en_pixeles(&previa, &camera);
                    (TAA_PESO_MINIMO
                        + (1.0 - TAA_PESO_MINIMO) * (px / TAA_MOVIMIENTO_MAXIMO))
                        .min(1.0)
                }
            }
        };
        let t_acumular = std::time::Instant::now();
        framebuffer.acumular(peso, &camera, camara_anterior.as_ref(), jitter);
        let ms_acumular = t_acumular.elapsed().as_secs_f32() * 1000.0;
        camara_anterior = Some(camera);

        let t_subida = std::time::Instant::now();
        texture
            .update_texture(framebuffer.to_rgba_bytes())
            .expect("no se pudo actualizar la textura");
        let ms_subida = t_subida.elapsed().as_secs_f32() * 1000.0;

        // Solo el trazado, no el vsync ni el dibujado: es lo unico que
        // depende de la escena y del antialiasing.
        let costo = empezo.elapsed().as_secs_f32();
        cuadro_medio = cuadro_medio * 0.8 + costo * 0.2;

        // El desglose se imprime en el primer cuadro y cada vez que se
        // toca el antialiasing. Sirve para saber DONDE se va el tiempo sin
        // tener que adivinarlo: el trazado es lo unico que depende de la
        // escena, y si el que pesa fuera otro, optimizar la escena no
        // serviria de nada.
        // En el cuadro 30 y no en el primero: los primeros cuadros tienen
        // las caches frias y el procesador todavia subiendo de frecuencia,
        // y dan un numero que no es el que se va a ver el resto del rato.
        // Medir ahi manda a optimizar lo que no es.
        if aviso_cuadro && cuadros > 30 {
            aviso_cuadro = false;
            let ms = (costo * 1000.0).max(1.0);
            println!(
                "    cuadro: {RENDER_W}x{RENDER_H} {} en {ms:.0} ms  (~{:.0} por segundo)",
                if antialias { "con AA 2x2" } else { "sin AA" },
                1000.0 / ms
            );
            println!(
                "      trazado {ms_trazado:.1} ms | acumulador {ms_acumular:.1} ms |                  subida a la GPU {ms_subida:.1} ms"
            );
        }

        // ---------- POST-PROCESADO EN LA GPU ----------
        //
        // Toda la cadena menos la ultima pasada escribe en buffers propios,
        // asi que va afuera del dibujado de la ventana:
        //
        //   trazado 400x300 -> bloom (5 pasadas, 200x150)
        //                   -> composite     (a `escena`,  800x600)
        //                   -> god rays      (a `rayos`,   800x600)
        //                   -> caleidoscopio (a `plegada`, 800x600)
        //
        // El radio del halo lo pone la cancion: apretado en la intro,
        // derramado en el coro final. El caleidoscopio pliega solo el fondo
        // (usa la profundidad que el trazado dejo en el alpha como
        // mascara), asi que el escenario se ve entero todo el tiempo.
        post.armar_bloom(
            &mut rl,
            &thread,
            &texture,
            params.bloom_radius,
            params.bloom_threshold,
        );
        post.componer(&mut rl, &thread, &texture, &params);
        // Los rayos bajan desde la luz cenital: se proyecta con la camara
        // de ESTE cuadro, la misma que acaba de trazar.
        post.god_rays(
            &mut rl,
            &thread,
            proyectar_a_pantalla(&camera, lights[0].position),
            params.pulso,
        );
        post.caleidoscopio(&mut rl, &thread, &texture, &params);

        // ---------- DIBUJADO ----------
        let mut d = rl.begin_drawing(&thread);
        d.clear_background(BACKGROUND);

        // La unica pasada que toca la pantalla: separa los canales hacia los
        // bordes y tira grano encima.
        // El foco esta en el centro de la fuente: a la distancia de la
        // camara, en la escala del depth buffer.
        //
        // Se calcula SIN el empujon del beat. Ese empujon es una sacudida
        // de camara, no una decision de encuadre, y un foquista no le
        // corre atras a cada tiempo: persiguiendolo, el plano de foco
        // temblaba a ritmo de negra.
        let foco = orbita.distancia_de_foco(params.cine) / Framebuffer::PROFUNDIDAD_MAXIMA;
        post.efectos(&mut d, &texture, &params, tiempo, foco);

        // EL HUD NO SALE EN LAS FOTOS.
        //
        // El modo `--foto` existe para sacar la imagen final de la escena
        // —para el informe, para el README, para mirarla— y una imagen con
        // los cuadros por segundo y los milisegundos por cuadro encima no
        // es la escena: es una captura de pantalla de un programa. Se
        // estuvo sacando asi durante todo el desarrollo y todas las
        // capturas quedaron con el contador quemado en la esquina.
        //
        // En vivo el HUD se queda, que es donde sirve: es el numero que
        // dice si la escena esta pesada, y sin verlo la unica forma de
        // saberlo es contar los saltos a ojo.
        if foto.is_none() {
            // El HUD baja para no quedar encima de la banda del formato ancho.
            let hud = 10 + (LETTERBOX_ALTO * params.cine * HEIGHT as f32) as i32;
            d.draw_fps(10, hud);
            d.draw_text(
                &format!(
                    "{:>3.0}s / {:.0}s   {}",
                    tiempo.rem_euclid(analisis.duracion.max(1.0)),
                    analisis.duracion,
                    if reloj.hay_musica() { "" } else { "(sin musica)" }
                ),
                10,
                hud + 24,
                18,
                Color::new(180, 120, 200, 255),
            );
            // Lo que cuesta el cuadro, a la vista. Es el numero que dice si la
            // escena esta pesada, y sin verlo la unica forma de saberlo es
            // contar los saltos a ojo.
            d.draw_text(
                &format!(
                    "{RENDER_W}x{RENDER_H} -> {WIDTH}x{HEIGHT}  ({:.0} ms)",
                    cuadro_medio * 1000.0
                ),
                10,
                hud + 46,
                18,
                Color::new(120, 150, 190, 255),
            );
        }

        // La ayuda: los primeros segundos y cuando se pide con H. Se
        // desvanece sola para no ensuciar la fuente.
        let desde_arranque = arranque.elapsed().as_secs_f32();
        if ayuda && foto.is_none() {
            let alpha = if desde_arranque < 8.0 {
                255
            } else if desde_arranque < 10.0 {
                ((10.0 - desde_arranque) / 2.0 * 255.0) as u8
            } else {
                0
            };
            if alpha > 0 {
                let fondo = Color::new(8, 6, 20, (alpha as u32 * 170 / 255) as u8);
                d.draw_rectangle(WIDTH as i32 - 250, 10, 240, 132, fondo);
                let texto = Color::new(220, 190, 240, alpha);
                let tenue = Color::new(160, 140, 190, alpha);
                d.draw_text("Great Fairy Fountain", WIDTH as i32 - 240, 18, 18, texto);
                for (i, linea) in [
                    "mouse / flechas: orbitar",
                    "rueda / Q E: acercar",
                    "espacio: pausa   X/T: antialias",
                    "F: foto   H: ocultar ayuda",
                ]
                .iter()
                .enumerate()
                {
                    d.draw_text(linea, WIDTH as i32 - 240, 46 + i as i32 * 22, 16, tenue);
                }
            }
            if desde_arranque >= 10.0 {
                // Ya se fue sola; H la vuelve a traer.
                ayuda = false;
            }
        }

        drop(d);
        cuadros += 1;

        if guardar_foto {
            let nombre = match foto {
                Some(t) => format!("foto_t{t:.0}.png"),
                None => format!("foto_{:.0}s.png", tiempo),
            };
            rl.take_screenshot(&thread, &nombre);
            println!("foto guardada: {nombre}");
            if foto.is_some() {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// La orbita inicial deja el ojo de frente, elevado y adentro de la
    /// cueva, mirando al centro.
    #[test]
    fn la_orbita_inicial_esta_de_frente() {
        let cam = Orbita::inicial().camara(2.0);

        // radio * sin(phi) y radio * cos(phi) sobre el punto de mira. Se
        // calculan de las constantes y no se escriben a mano: este test se
        // rompio una vez porque tenia el radio viejo clavado.
        let r = Orbita::RADIO_INICIAL;
        let ph = Orbita::PHI_BASE;
        assert!((cam.position.x).abs() < 1e-4);
        assert!((cam.position.y - (CAMARA_MIRA.y + r * ph.sin())).abs() < 0.05);
        assert!((cam.position.z - r * ph.cos()).abs() < 0.05);

        // Mira hacia -Z y un poco hacia abajo.
        let f = cam.get_forward();
        assert!(f.z < -0.9 && f.y < 0.0);
    }

    /// Los topes: ni el pendulo ni las teclas sacan el angulo de su rango,
    /// y el radio tampoco se pasa.
    #[test]
    fn la_orbita_respeta_los_topes() {
        let mut o = Orbita::inicial();
        o.avanzar(0.0, 10.0, 10.0, 100.0);
        assert_eq!(o.theta(), Orbita::THETA_MAX);
        assert_eq!(o.phi(), Orbita::PHI_MAX);
        assert_eq!(o.radio, Orbita::RADIO_MAX);

        o.avanzar(0.0, -20.0, -20.0, -200.0);
        assert_eq!(o.theta(), -Orbita::THETA_MAX);
        assert_eq!(o.phi(), Orbita::PHI_MIN);
        assert_eq!(o.radio, Orbita::RADIO_MIN);

        // El corrimiento manual no acumula mas alla del tope: apenas se
        // aprieta la otra tecla, la camara responde.
        let antes = o.theta();
        o.avanzar(0.0, 0.03, 0.0, 0.0);
        assert!(o.theta() > antes);
    }

    /// Sin tocar nada, el recorrido entero queda DELANTE de la fuente y
    /// adentro de la cueva: fuera del techo (que llega a |x|, |z| = 4),
    /// sobre el piso (30 x 30) y lejos de la pared del fondo (z = -11).
    #[test]
    fn el_pendulo_no_sale_de_la_escena() {
        let mut o = Orbita::inicial();
        let paso = 0.1;
        for _ in 0..(120.0 / paso) as usize {
            o.avanzar(paso, 0.0, 0.0, 0.0);
            let p = o.camara(2.0).position;
            assert!(p.x.abs() < 14.0, "x = {}", p.x);
            assert!(p.z > 4.5 && p.z < 14.0, "z = {}", p.z);
            assert!(p.y > 0.5 && p.y < 8.0, "y = {}", p.y);
        }
    }

    /// El pendulo oscila de verdad: llega cerca de los dos extremos y pasa
    /// por el centro, con la altura respirando apenas.
    #[test]
    fn el_pendulo_oscila() {
        let mut o = Orbita::inicial();
        let (mut min_t, mut max_t) = (f32::INFINITY, f32::NEG_INFINITY);
        let (mut min_p, mut max_p) = (f32::INFINITY, f32::NEG_INFINITY);
        for _ in 0..600 {
            o.avanzar(0.1, 0.0, 0.0, 0.0);
            min_t = min_t.min(o.theta());
            max_t = max_t.max(o.theta());
            min_p = min_p.min(o.phi());
            max_p = max_p.max(o.phi());
        }
        // Los umbrales se escriben contra la CONSTANTE y no contra un
        // numero suelto: asi el test sigue comprobando lo que le importa
        // (que el pendulo recorra casi toda su amplitud a los dos lados)
        // si alguien vuelve a mover `THETA_AMPLITUD`, en vez de romperse.
        // Se rompio una vez por esto, cuando la amplitud bajo de 0.65 a
        // 0.36 para sacar las columnas del centro del cuadro.
        let casi = Orbita::THETA_AMPLITUD * 0.98;
        assert!(min_t < -casi && max_t > casi, "theta: {min_t} a {max_t}");
        assert!(min_p < 0.21 && max_p > 0.29, "phi: {min_p} a {max_p}");
    }

    /// Desde cualquier punto del recorrido sigue mirando al centro, a la
    /// distancia del radio.
    #[test]
    fn la_orbita_mira_al_centro() {
        let mut o = Orbita::inicial();
        for _ in 0..40 {
            o.avanzar(0.7, 0.0, 0.0, 0.0);
            let cam = o.camara(2.0);

            let d = (cam.position - CAMARA_MIRA).magnitude();
            assert!((d - o.distancia()).abs() < 1e-3);
            assert!((d - Orbita::RADIO_INICIAL).abs() <= Orbita::RADIO_AMPLITUD + 1e-3);

            let hacia_centro = normalize(&(CAMARA_MIRA - cam.position));
            assert!(dot(&hacia_centro, &cam.get_forward()) > 0.999);
        }
    }
}
