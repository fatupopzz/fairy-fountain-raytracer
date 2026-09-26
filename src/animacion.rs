//! El puente entre la linea de tiempo y la escena de verdad.
//!
//! `sync.rs` dice COMO tiene que estar la escena en el segundo t. Este
//! modulo es el que lo hace: agarra esos multiplicadores y los escribe
//! sobre las hadas y las luces que ya estan construidas.
//!
//! Los objetos se ubican por INDICE, anotado mientras se arma la escena.
//! Con el trait objeto no se llega al tipo concreto, asi que se baja con
//! `Any` (ver el supertrait en `ray_intersect.rs`) y si el downcast falla no
//! se toca nada: es preferible que una pieza se quede quieta a mover la
//! equivocada.
//!
//! NADA de aca acumula. Cada cuadro se calcula sobre el valor ORIGINAL con
//! el que se construyo la pieza, nunca sobre el del cuadro anterior. Si se
//! leyera el anterior, un multiplicador de 0.9 sostenido apagaria la escena
//! hasta dejarla negra en un minuto.

use crate::cylinder::CilindroOrientado;
use crate::grupo_acotado::GrupoAcotado;
use crate::light::Light;
use crate::plane::Plane;
use crate::ray_intersect::RayIntersect;
use crate::sphere::Sphere;
use crate::sync::SceneParams;
use crate::toro::Toro;
use crate::triangle::Triangle;
use crate::vec3::Vec3;
use raylib::prelude::Color;
use std::any::Any;

/// Todo lo que hay que poder volver a encontrar despues de armar la escena.
pub struct EscenaViva {
    /// Los grupos acotados cuyos hijos son TODOS orbes de hada (esferas),
    /// con el centro, el radio y la emision con los que nacio cada hijo, en
    /// orden.
    ///
    /// Van por grupo y no por esfera suelta porque las hadas se agrupan
    /// para acelerar el trazado: al grupo se le pide los hijos y se les
    /// escribe a todos, que es justo lo que hace falta. Los grupos tienen
    /// que haberse armado con `GrupoAcotado::con_margen(.., HADA_ALCANCE)`,
    /// porque las hadas se MUEVEN y la esfera acotante no se recalcula.
    orbes: Vec<(usize, Vec<(Vec3, f32, Color)>)>,
    /// EL GRUPO DE LAS ESTELAS: las que cruzan la fuente con el arpa.
    ///
    /// Un solo grupo con `ESTELAS * ESTELA_SEGMENTOS` esferas adentro,
    /// repartidas de a bloques: los primeros `ESTELA_SEGMENTOS` hijos son
    /// la primera estela, los siguientes la segunda, y asi. Se arma con
    /// margen, como las hadas, porque cruzan la escena entera y la caja
    /// no se recalcula.
    estelas: Vec<usize>,
    /// EL GRUPO DE LOS ANILLOS que rodean la Trifuerza: los toros. Un solo
    /// grupo con los `ANILLOS` adentro, armado con margen porque los
    /// anillos GIRAN y la caja del arbol se arma una sola vez.
    anillos: Vec<usize>,
    /// La intensidad con la que se construyo cada luz puntual.
    luces_base: Vec<f32>,
    /// Donde esta el agua: `(indice del grupo, indice del hijo)`. Es un
    /// `Plane` adentro del grupo de la piscina, y cada cuadro se le
    /// escriben la fuerza y la fase del oleaje.
    agua: Option<(usize, usize)>,
    /// La emision con la que nacio el agua, para respirar sobre ella.
    agua_emision: Option<Color>,
    /// La Triforce: el grupo del altar y, adentro, los triangulos que
    /// pulsan con el ritmo, con la emision con la que nacieron.
    triforce: Option<(usize, Vec<(usize, Color)>)>,
    /// Los grupos de polvo de hada, con el centro de nacimiento de cada
    /// mota. Derivan despacio por el aire.
    polvo: Vec<(usize, Vec<Vec3>)>,
    /// Las rupias: para cada una, su grupo, el eje sobre el que gira y los
    /// tres vertices con los que nacio cada triangulo.
    rupias: Vec<Rupia>,
    /// Las luces que viven adentro de los cristales: su grupo, cual de los
    /// hijos es la esfera, su color de nacimiento y que region del circulo
    /// de quintas le toca.
    cristales: Vec<(usize, usize, Color, usize)>,
}

/// Una rupia que gira sobre su eje y flota.
struct Rupia {
    grupo: usize,
    centro: Vec3,
    /// Los vertices de cada cara RELATIVOS al centro, como nacieron.
    caras: Vec<(Vec3, Vec3, Vec3)>,
    /// Corrimiento de fase, para que no giren ni floten todas juntas.
    fase: f32,
}

impl EscenaViva {
    pub fn nueva() -> Self {
        EscenaViva {
            orbes: Vec::new(),
            estelas: Vec::new(),
            anillos: Vec::new(),
            luces_base: Vec::new(),
            agua: None,
            agua_emision: None,
            triforce: None,
            polvo: Vec::new(),
            rupias: Vec::new(),
            cristales: Vec::new(),
        }
    }

    /// Anota la Triforce: el grupo `grupo` y los hijos que son sus
    /// triangulos, con su emision de nacimiento.
    pub fn registrar_triforce(&mut self, grupo: usize, hijos: Vec<(usize, Color)>) {
        self.triforce = Some((grupo, hijos));
    }

    /// Anota una rupia: su grupo, su centro, y los vertices de sus caras
    /// en el orden en que se metieron al grupo.
    pub fn registrar_rupia(&mut self, grupo: usize, centro: Vec3, caras: Vec<(Vec3, Vec3, Vec3)>, fase: f32) {
        let caras = caras
            .into_iter()
            .map(|(a, b, c)| (a - centro, b - centro, c - centro))
            .collect();
        self.rupias.push(Rupia { grupo, centro, caras, fase });
    }

    /// Anota la luz de adentro de un cristal: el grupo, cual hijo es la
    /// esfera emisiva, su color y la region armonica que le toca.
    pub fn registrar_cristal(&mut self, grupo: usize, hijo: usize, color: Color, region: usize) {
        self.cristales.push((grupo, hijo, color, region));
    }

    /// Anota un grupo de motas de polvo con el centro de cada una, en el
    /// orden en que se metieron al grupo. El grupo tiene que haberse armado
    /// con margen `POLVO_ALCANCE`.
    pub fn registrar_polvo(&mut self, grupo: usize, centros: Vec<Vec3>) {
        self.polvo.push((grupo, centros));
    }

    /// Anota donde quedo el plano del agua: el hijo `hijo` del grupo que
    /// va a estar en `objetos[grupo]`.
    pub fn registrar_agua(&mut self, grupo: usize, hijo: usize, emision: Option<Color>) {
        self.agua = Some((grupo, hijo));
        self.agua_emision = emision;
    }

    /// Anota un grupo de orbes de hada. `bases` (centro, radio y emision de
    /// cada una) va en el mismo orden en que se metieron los hijos al grupo.
    pub fn registrar_orbes(&mut self, indice: usize, bases: Vec<(Vec3, f32, Color)>) {
        self.orbes.push((indice, bases));
    }

    /// Se llama UNA vez, con las luces ya construidas.
    /// Anota el grupo de UNA estela del arpa. Se llama una vez por ranura.
    pub fn registrar_estela(&mut self, grupo: usize) {
        self.estelas.push(grupo);
    }

    /// Anota el grupo de los anillos de la Trifuerza.
    pub fn registrar_anillos(&mut self, grupo: usize) {
        self.anillos.push(grupo);
    }

    pub fn registrar_luces(&mut self, luces: &[Light]) {
        self.luces_base = luces.iter().map(|l| l.intensity).collect();
    }
}

/// Escala un color de emision, recortando en 255.
///
/// OJO con lo que este recorte significa: la emision vive en un `Color` de
/// 8 bits, asi que "emision 2.0" no puede guardarse como dos veces el color.
/// Las hadas nacen con la emision al tope (255, 204, 255), asi que el
/// multiplicador de la cancion trabaja por DEBAJO de 1.0: las apaga un poco
/// en el silencio y las deja a full en los arpegios. El "mas de uno" lo pone
/// el BLOOM, que tiene el umbral bajo y el radio grande justo para eso: el
/// que se ve floreciendo es el halo, no el pixel.
fn escalar(color: Color, factor: f32) -> Color {
    let canal = |c: u8| (c as f32 * factor).clamp(0.0, 255.0) as u8;

    Color::new(canal(color.r), canal(color.g), canal(color.b), 255)
}

/// Hasta donde puede alejarse un hada de donde nacio, en cualquier eje.
/// Es el margen con el que hay que armar sus grupos acotados: la caida
/// entera mas la deriva.
pub const HADA_ALCANCE: f32 = HADA_CAIDA + HADA_DERIVA + HADA_VAIVEN + HADA_NOTA_SUBE + 0.05;

/// Hasta donde cae un hada desde donde nacio. Al llegar se queda flotando
/// ahi hasta que el arpa la deshace. 0.8 deja a las del anillo bajo (que
/// nacen en y = 1.3) a medio metro del agua, y a las doradas por encima
/// del pedestal.
const HADA_CAIDA: f32 = 0.8;
/// A que velocidad caen, en unidades por segundo. Lento: es polvo de hada
/// bajando, no lluvia. La caida entera lleva unos trece segundos.
const HADA_VELOCIDAD: f32 = 0.06;
/// Amplitud de la deriva lenta, a los costados.
const HADA_DERIVA: f32 = 0.1;
/// Cuanto tarda un hada en deshacerse cuando le toca su nota, en segundos.
/// Era medio segundo: ahora se va despacio, como se apaga una brasa.
const HADA_DISOLUCION: f32 = 1.2;
/// Cuanto tarda en volver a aparecer arriba, creciendo desde nada.
const HADA_RENACER: f32 = 3.0;
/// Cuanto se mecen arriba y abajo al tempo, en unidades.
const HADA_VAIVEN: f32 = 0.06;

/// Cuanto SUBE un hada cuando suena su nota, en unidades. Poco: lo que se
/// tiene que ver es el brillo recorriendo el anillo, y el movimiento esta
/// para acompaniarlo, no para que parezcan saltando.
const HADA_NOTA_SUBE: f32 = 0.22;

/// Cuanto le suma a su brillo, y cuanto le engorda el radio.
const HADA_NOTA_BRILLO: f32 = 0.85;
const HADA_NOTA_TAMANIO: f32 = 0.45;

/// Cuanto llega a brillar la luz de un cristal cuando su region esta
/// sonando, y cuanto de eso es piso: ni apagados del todo ni encendidos
/// del todo. El cristal es refractivo, asi que lo que se ve no es la
/// esfera sino su luz deformada a traves de las caras.
const CRISTAL_PISO: f32 = 0.35;
const CRISTAL_RANGO: f32 = 1.05;

/// Con que nota se queda el hada numero `k`.
///
/// El salto de siete semitonos entre hadas consecutivas NO es decorativo:
/// siete semitonos son una quinta, o sea el CIRCULO DE QUINTAS, y como 7 y
/// 12 no tienen divisores comunes, las doce notas se reparten sin repetir
/// antes de dar la vuelta.
///
/// Por que importa: las notas de un acorde estan cerca en el circulo de
/// quintas y lejos en la escala. Repartiendo asi, un acorde enciende hadas
/// VECINAS (se lee como un grupo que se prende junto) mientras que dos
/// notas que chocan al oido caen en lados opuestos de la fuente. Con las
/// notas en orden de escala pasaba lo contrario: los acordes encendian
/// hadas desparramadas y no se leia ninguna figura.
fn nota_de(k: usize) -> usize {
    (k * 7) % 12
}

/// Cuanto sube y baja una rupia al flotar, en unidades. Es tambien el
/// margen que necesitan sus grupos acotados: girar sobre el eje propio no
/// cambia el volumen, flotar si.
pub const RUPIA_FLOTE: f32 = 0.12;

/// Cuanto tarda una rupia en dar una vuelta entera, en segundos. Lento: la
/// rupia tiene que destellar cuando la cara alcanza a la luz, no girar
/// como un trompo.
const RUPIA_VUELTA: f32 = 9.0;

/// Cuantos radianes de fase le suma la energia del tema al giro. Dos son
/// un tercio de vuelta entre el silencio y el coro: mientras el tema
/// crece se las ve apurarse, y cuando afloja, frenar.
const RUPIA_APURO: f32 = 2.0;

/// Hasta donde se aleja una mota de polvo de donde nacio, en cualquier
/// eje: el margen de sus grupos acotados.
pub const POLVO_ALCANCE: f32 = 0.45;

/// A que velocidad viajan los anillos del agua, en radianes de fase por
/// segundo. Con la escala de 3 anillos por unidad, 2.4 rad/s es una ola
/// que cruza la piscina en unos ocho segundos: lento, es una fuente y no
/// una playa.
pub const AGUA_VELOCIDAD: f32 = 1.5;

/// Donde esta y cuanto se ve el hada numero `k` (de `total`) en el segundo
/// `tiempo`, partiendo de donde nacio: `(posicion, presencia)`, con
/// presencia entre 0 (deshecha) y 1 (entera).
///
/// LAS HADAS SE DESHACEN CON EL ARPA. Cada ataque del arpa le toca a un
/// hada (el ataque numero j a la hada `(j * 7) % total`: el 7 es coprimo
/// con la cantidad de hadas, asi que las notas saltan por la fuente en vez
/// de recorrer el anillo en orden) y esa hada se deshace donde este: se
/// encoge hasta desaparecer en medio segundo, y renace arriba, en su
/// lugar de nacimiento, creciendo despacio. Desde ahi cae a velocidad
/// constante hasta `HADA_CAIDA` y se queda flotando abajo hasta que le
/// toque la proxima nota. Encima va una deriva lateral muy lenta (senos de
/// periodos distintos en x y z) para que no bajen en linea recta.
///
/// Nada salta a la vista: el unico cambio brusco de posicion (de abajo a
/// arriba) pasa con la presencia en cero.
///
/// Es funcion pura del tiempo y de la lista de ataques, sin estado: se
/// busca cual fue la ultima nota de esta hada y, para saber desde donde
/// se deshizo, la anterior. Si la anterior no entra en la ventana de
/// ataques que manda el analisis, se asume que ya habia llegado abajo
/// (la caida entera dura mucho menos que la ventana).
fn hada_en(base: Vec3, k: usize, total: usize, tiempo: f32, ataques: &[(usize, f32)]) -> (Vec3, f32) {
    let fase = k as f32 * 0.37;
    let deriva = Vec3::new(
        (tiempo * 0.31 + fase * 7.0).sin() * HADA_DERIVA,
        0.0,
        (tiempo * 0.27 + fase * 5.0).cos() * HADA_DERIVA,
    );
    let suave = |x: f32| {
        let x = x.clamp(0.0, 1.0);
        x * x * (3.0 - 2.0 * x)
    };
    let caida = |segundos: f32| (HADA_VELOCIDAD * segundos.max(0.0)).min(HADA_CAIDA);

    // Las dos ultimas notas de esta hada: la ultima y la anterior.
    let mut mias = ataques
        .iter()
        .filter(|(j, t)| (j * 7) % total.max(1) == k && *t <= tiempo)
        .map(|(_, t)| *t);
    let anterior_y_ultima = {
        let mut a = None;
        let mut b = None;
        for t in mias.by_ref() {
            a = b;
            b = Some(t);
        }
        (a, b)
    };

    let (posicion, presencia) = match anterior_y_ultima {
        // Todavia no le toco ninguna nota: viene cayendo desde el principio.
        (_, None) => (base - Vec3::new(0.0, caida(tiempo), 0.0), 1.0),

        (anterior, Some(ultima)) => {
            let desde = tiempo - ultima;

            if desde < HADA_DISOLUCION {
                // Deshaciendose donde estaba cuando llego la nota: lo que
                // habia caido desde que renacio la vez anterior o, si no
                // hubo vez anterior, desde el principio del tema (que, si
                // el tema ya lleva mas que la ventana, da la caida entera:
                // es lo que se asume cuando la nota anterior quedo afuera).
                let caido = match anterior {
                    Some(t_prev) => caida(ultima - t_prev - HADA_DISOLUCION),
                    None => caida(ultima),
                };
                (
                    base - Vec3::new(0.0, caido, 0.0),
                    1.0 - suave(desde / HADA_DISOLUCION),
                )
            } else {
                // Renaciendo arriba y volviendo a caer.
                let vivo = desde - HADA_DISOLUCION;
                (
                    base - Vec3::new(0.0, caida(vivo), 0.0),
                    suave(vivo / HADA_RENACER),
                )
            }
        }
    };

    (posicion + deriva, presencia)
}

/// Deja la escena en el estado que le toca al segundo `tiempo`.
///
/// `params` YA viene con las capas aplicadas (`sync::get_scene_params` se
/// encarga): estructura por secciones, pulsos del ritmo y destellos. Aca
/// solo se escriben los numeros sobre la escena.
/// CUANTAS ESTELAS PUEDEN CRUZAR A LA VEZ.
///
/// Tres. Saliendo en el tiempo fuerte (ver `SyncData::estelas_hasta`) la mas
/// seguida son dos estelas por compas y medio, o sea una cada 1.8 segundos
/// contra una vida de `ESTELA_VIDA`: medido sobre la cancion nunca hay mas
/// de una cruzando. Las otras dos ranuras son el margen para el dia que la
/// vida crezca o el criterio se apure, porque quedarse sin ranura no se ve
/// como una estela menos sino como una que desaparece a mitad de vuelo.
pub const ESTELAS: usize = 3;

/// Cuanto dura el cruce, en segundos.
pub const ESTELA_VIDA: f32 = 1.5;

/// Cuanto mide el recorrido, en unidades del mundo. La fuente mide seis de
/// lado y la plaza doce: catorce la cruza entera y sobra para entrar y
/// salir fuera de cuadro.
pub const ESTELA_LARGO: f32 = 14.0;

/// De que esta hecha cada estela: UNA CABEZA Y UNA COLA.
///
/// La cabeza es una esfera y la cola un cilindro orientado que va de la
/// cabeza hacia atras. Dos primitivas, no mas.
///
/// Se probo primero con una fila de diez esferitas solapadas y no
/// funciona: por mas que se solapen, cada esfera tiene su silueta y su
/// sombreado, asi que la estela se lee como una ORUGA con festones. Un
/// cilindro no tiene costuras porque es una sola superficie, y de paso
/// cuesta una decima parte.
/// El radio de la cabeza.
const ESTELA_RADIO: f32 = 0.10;

/// El radio de la cola, mas fino que la cabeza: asi la estela tiene punta.
const ESTELA_RADIO_COLA: f32 = 0.055;

/// Cuanto mide la cola, en unidades del mundo.
const ESTELA_COLA: f32 = 1.6;

/// Hasta donde puede llegar una estela desde el centro de la escena: es el
/// margen con el que hay que armar su grupo acotado.
pub const ESTELA_ALCANCE: f32 = ESTELA_LARGO * 0.5 + 1.0;

/// LA ESTELA NUMERO `n` EN EL INSTANTE `edad`.
///
/// Devuelve, para cada segmento, donde esta y cuanto brilla. Todo sale del
/// numero de beat que la lanzo pasado por un hash, igual que las estrellas
/// fugaces del cielo y por la misma razon: asi la escena sigue siendo
/// funcion del segundo en el que estamos y no de cuantos cuadros se hayan
/// dibujado. El `n` es el numero de beat y no el de ataque desde que las
/// estelas salen en el compas, pero para el hash da igual: lo unico que le
/// pide es que sea el MISMO numero cada vez que se dibuje esa estela.
///
/// EL RECORRIDO CRUZA ENTRE LAS COLUMNAS. Entra por un punto de un circulo
/// de radio siete —fuera del anillo de columnas, que esta en 3.5— y sale
/// por el otro lado, pasando a una distancia del centro que tambien sale
/// del hash: algunas rozan la Triforce y otras pasan de largo por un
/// costado. La altura va entre el borde de la piscina y el techo, que es
/// la banda por donde se ve el hueco entre columna y columna.
fn estela_en(n: u32, edad: f32) -> (Vec3, Vec3, f32) {
    // LOS DIECISEIS BITS DE ARRIBA, no los de abajo. Con `n` yendo de cuatro
    // en cuatro (son numeros de beat, y las estelas salen en el uno de cada
    // compas) los bits bajos entran casi sin informacion y esta mezcla no
    // alcanza a repartirlos: tomando los de abajo, de las 54 estelas del
    // tema veintitres entraban por el mismo cuadrante y tres por el de
    // enfrente, o sea que casi todas cruzaban para el mismo lado. Los de
    // arriba son los que se llevan la multiplicacion entera y quedan doce o
    // catorce por cuadrante.
    let h = |k: u32| {
        let mut x = n.wrapping_mul(0x9e37_79b9) ^ k.wrapping_mul(0x85eb_ca6b);
        x ^= x >> 15;
        x = x.wrapping_mul(0x2545_f491);
        x ^= x >> 13;
        ((x >> 16) & 0xFFFF) as f32 / 65535.0
    };

    let fraccion = (edad / ESTELA_VIDA).clamp(0.0, 1.0);

    let angulo = h(1) * std::f32::consts::TAU;
    let (sa, ca) = angulo.sin_cos();
    // Cuanto se aparta del centro: algunas rozan la Triforce y otras pasan
    // de largo por un costado.
    let impacto = (h(2) - 0.5) * 4.0;
    let altura = 1.3 + h(3) * 2.6;
    let caida = (h(4) - 0.5) * 1.2;

    let entrada = Vec3::new(ca * 7.0 - sa * impacto, altura, sa * 7.0 + ca * impacto);
    let marcha = Vec3::new(-ca, caida / ESTELA_LARGO, -sa);

    let avance = fraccion * ESTELA_LARGO;
    let cabeza = entrada + marcha * avance;
    // La cola no puede salirse por detras del punto de entrada, o al
    // principio se veria asomar de la nada.
    let cola = entrada + marcha * (avance - ESTELA_COLA).max(0.0);

    // Entra y sale con una curva en S: ni aparece ni desaparece de golpe.
    let x = (1.0 - (fraccion * 2.0 - 1.0).abs()).clamp(0.0, 1.0);
    (cabeza, cola, x * x * (3.0 - 2.0 * x))
}

/// Escribe las estelas del arpa sobre sus esferas.
fn actualizar_estelas(
    objetos: &mut [Box<dyn RayIntersect + Send + Sync>],
    escena: &EscenaViva,
    params: &SceneParams,
) {
    // Cuales estan vivas AHORA. Las salidas llegan en los parametros, que ya
    // eligieron los tiempos fuertes que lanzan una (ver
    // `SyncData::estelas_hasta`); aca solo se descarta lo que ya cruzo, asi
    // que esto sigue siendo funcion pura del segundo en el que estamos.
    let mut vivas: Vec<(u32, f32)> = params
        .estelas
        .iter()
        .filter_map(|&(n, t)| {
            let edad = params.tiempo - t;
            (0.0..ESTELA_VIDA).contains(&edad).then_some((n as u32, edad))
        })
        .collect();
    // Si hubiera mas que ranuras, se quedan las mas recientes.
    vivas.sort_by(|a, b| a.1.total_cmp(&b.1));
    vivas.truncate(ESTELAS);

    // Los tres colores de las hadas: la estela es una de ellas cruzando.
    const PALETA: [(f32, f32, f32); 3] =
        [(1.0, 0.55, 0.85), (0.45, 0.85, 1.0), (1.0, 0.85, 0.45)];

    for (ranura, &indice) in escena.estelas.iter().enumerate() {
        let Some(objeto) = objetos.get_mut(indice) else {
            continue;
        };
        let Some(grupo) = (objeto.as_mut() as &mut dyn Any).downcast_mut::<GrupoAcotado>() else {
            continue;
        };
        let datos = vivas.get(ranura).copied();
        let hijos = grupo.children_mut();

        let (cabeza, cola, vida, grosor, color_cabeza, color_cola) = match datos {
            None => (Vec3::zeros(), Vec3::zeros(), 0.0, 1.0, Color::BLACK, Color::BLACK),
            Some((n, edad)) => {
                let (cab, col, vida) = estela_en(n, edad);
                // Por COMPAS, no por beat: `n` va de cuatro en cuatro, asi
                // que dividirlo por los cuatro tiempos hace que dos estelas
                // seguidas sean siempre dos hadas distintas.
                let (r, g, b) = PALETA[(n as usize / 4) % PALETA.len()];
                // EL TOPE CRECE CON EL TEMA, de 0.70 a 0.95.
                //
                // El 0.70 esta para que se vea el COLOR: a pleno, el bloom
                // lleva la cabeza a blanco y la estela pierde de que hada
                // era. Pero fijo en 0.70 la estela del segundo 20 y la del
                // climax pesan lo mismo, y el resto de la escena no: las
                // luces, el bloom, la camara y la profundidad de campo
                // crecen todas con `cine`. Una estela que no crece con
                // ellas se va HACIA ATRAS en el cuadro a medida que el tema
                // se agranda, que es lo contrario de lo que tiene que
                // hacer lo que lleva el arpa.
                let tope = 0.70 + 0.25 * params.cine;
                let tinte = |k: f32| {
                    let c = |x: f32| (x * 255.0 * vida * k * tope).clamp(0.0, 255.0) as u8;
                    Color::new(c(r), c(g), c(b), 255)
                };
                // Y ENGORDAN: la mitad mas de radio sobre el final. Es lo
                // que se nota de verdad contra un cuadro ya cargado de
                // bloom, porque el bloom pinta el halo pero la silueta la
                // decide el radio.
                let grosor = 1.0 + 0.5 * params.cine;
                (cab, col, vida, grosor, tinte(1.0), tinte(0.45))
            }
        };

        if let Some(h) = hijos.first_mut() {
            if let Some(e) = (h.as_mut() as &mut dyn Any).downcast_mut::<Sphere>() {
                e.center = cabeza;
                e.radius = ESTELA_RADIO * vida * grosor;
                e.material.emission_color = Some(color_cabeza);
            }
        }
        if let Some(h) = hijos.get_mut(1) {
            if let Some(cil) = (h.as_mut() as &mut dyn Any).downcast_mut::<CilindroOrientado>() {
                cil.set_visible(vida > 0.0);
                if vida > 0.0 {
                    cil.recolocar(cola, cabeza);
                    cil.set_radio(ESTELA_RADIO_COLA * vida * grosor);
                    cil.material_mut().emission_color = Some(color_cola);
                }
            }
        }

        grupo.recalcular_caja(0.02);
    }
}

/// CUANTOS ANILLOS RODEAN LA TRIFUERZA.
///
/// Tres. Con dos se lee como un adorno simetrico y con cuatro ya es una
/// jaula: el simbolo que hay adentro tiene que seguir siendo lo que se
/// mira. Ver `toro.rs` por que un anillo es un TORO y no una esfera
/// achatada.
pub const ANILLOS: usize = 3;

/// Del centro del agujero al centro del tubo. La Trifuerza mide 1.36 de
/// ancho; con 1.38 los anillos la ABRAZAN, que es lo que se busca. Se
/// probo con 1.5 y en cuadro se la tragan: el anillo pasa por delante del
/// agua y el ojo lee tres aros con algo adentro en vez de la Trifuerza
/// rodeada.
pub const ANILLO_RADIO: f32 = 1.38;

/// El grosor del tubo EN REPOSO. Fino: un anillo grueso tapa, y lo que
/// tiene que hacer es dibujar una linea de luz en el aire. Con el ataque
/// del arpa engorda (ver `actualizar_anillos`).
pub const ANILLO_GROSOR: f32 = 0.055;

/// Cuanto dura el destello de un anillo cuando le toca su ataque del arpa,
/// en segundos.
///
/// Corto. Un anillo que tarda en apagarse deja de leerse como un GOLPE y
/// pasa a ser una luz que sube y baja; y como los tres se reparten los
/// ataques, uno largo haria que casi siempre estuvieran los tres
/// encendidos, que es lo mismo que ninguno.
const ANILLO_DESTELLO: f32 = 0.45;

/// Hasta donde puede llegar un anillo desde el centro de la Trifuerza.
///
/// Es el margen con el que hay que armar su grupo, y tiene que contemplar
/// LOS DOS movimientos: que el anillo gira (asi que barre la esfera de su
/// radio) y que con el golpe se agranda y engorda. Si la caja de nacimiento
/// se quedara corta, el arbol recortaria justo en el golpe, que es cuando
/// se lo mira.
pub const ANILLO_ALCANCE: f32 = ANILLO_RADIO * 1.12 + ANILLO_GROSOR * 3.0;

/// A que altura viven, que es la del medio de la Trifuerza.
pub const ANILLO_Y: f32 = 1.95;

/// Los anillos giran: se les escribe el eje y la emision.
///
/// EL GIRO ES UNA PRECESION, no una vuelta. Cada anillo esta inclinado y su
/// eje da vueltas alrededor de la vertical, como un giroscopo: asi el
/// anillo cambia de silueta todo el tiempo (de circulo a elipse a linea) en
/// vez de girar sobre si mismo, que en un anillo liso no se veria.
///
/// Las tres inclinaciones y las tres velocidades son distintas y no son
/// multiplos: si lo fueran, los tres volverian a la misma pose cada tanto y
/// el ojo agarraria el ciclo.
///
/// La fase sale de `giro_hadas`, que se cuenta en BEATS: los anillos giran
/// al tempo de la cancion aunque el movimiento sea demasiado lento para
/// que se note a que velocidad va.
fn actualizar_anillos(
    objetos: &mut [Box<dyn RayIntersect + Send + Sync>],
    escena: &EscenaViva,
    params: &SceneParams,
) {
    // El color de cada anillo: los mismos tres de las hadas, para que se
    // lean como de la misma familia que todo lo que flota en la cueva.
    const PALETA: [(f32, f32, f32); ANILLOS] =
        [(1.0, 0.62, 0.88), (0.52, 0.88, 1.0), (1.0, 0.88, 0.55)];
    const INCLINACION: [f32; ANILLOS] = [0.42, 1.05, 1.62];
    const VELOCIDAD: [f32; ANILLOS] = [0.60, -0.37, 0.23];

    for &indice in &escena.anillos {
        let Some(objeto) = objetos.get_mut(indice) else {
            continue;
        };
        let Some(grupo) = (objeto.as_mut() as &mut dyn Any).downcast_mut::<GrupoAcotado>() else {
            continue;
        };

        for (k, hijo) in grupo.children_mut().iter_mut().enumerate().take(ANILLOS) {
            let Some(anillo) = (hijo.as_mut() as &mut dyn Any).downcast_mut::<Toro>() else {
                continue;
            };

            let fase = params.giro_hadas * VELOCIDAD[k] + k as f32 * 2.1;
            let (sf, cf) = fase.sin_cos();
            let (si, ci) = INCLINACION[k].sin_cos();
            anillo.set_eje(Vec3::new(si * cf, ci, si * sf));

            // CUANTO BRILLAN: la armonia de su region mas el golpe, y todo
            // escalado por `cine`. Al principio del tema son tres hilos
            // apenas visibles y sobre el final son de las cosas que mas
            // brillan, igual que el resto de la escena.
            let (r, g, b) = PALETA[k];
            // CADA ANILLO SE QUEDA CON UN ATAQUE DE CADA TRES.
            //
            // Antes los tres colgaban de `armonia` y de `pulso`, que son
            // dos cosas lentas y COMUNES a los tres: los tres subian y
            // bajaban juntos, y tres cosas que hacen lo mismo a la vez se
            // leen como una sola. Repartiendo los ataques del arpa de a
            // uno, el arpa RECORRE los anillos, que es lo mismo que hace el
            // arpegio con las hadas y lo que convierte tres aros en una
            // figura que toca.
            //
            // Es el mismo reparto por numero de ataque que usan las hadas,
            // asi que sigue siendo funcion pura del segundo en el que
            // estamos: el ataque 17 es siempre del mismo anillo.
            let destello = params
                .ataques
                .iter()
                .filter(|(n, _)| n % ANILLOS == k)
                .filter_map(|&(_, t)| {
                    let edad = params.tiempo - t;
                    (0.0..ANILLO_DESTELLO)
                        .contains(&edad)
                        .then(|| 1.0 - edad / ANILLO_DESTELLO)
                })
                .fold(0.0f32, f32::max);
            // Al cuadrado: pega y se va, en vez de bajar parejo.
            let destello = destello * destello;

            // LO QUE MAS SE VE NO ES EL BRILLO SINO LA SILUETA, que es la
            // leccion que dejaron las estelas del arpa. Con el golpe el
            // tubo ENGORDA hasta el doble y el anillo se ABRE un poco, como
            // una onda que sale de la Trifuerza; el brillo se monta encima.
            // Y por debajo de los dos, el anillo respira con la energia del
            // tema, que es el movimiento lento que los mantiene vivos
            // mientras el arpa calla.
            anillo.radio_menor = ANILLO_GROSOR
                * (1.0 + destello * 1.10 + params.pulso * 0.35);
            anillo.radio_mayor = ANILLO_RADIO
                * (1.0 + destello * 0.055 + params.energia_suave * 0.035);

            let fuerza = (0.20 + params.armonia[k] * 0.45 + destello * 0.85
                + params.pulso * 0.15)
                * (0.40 + 0.60 * params.cine);
            let canal = |x: f32| (x * 255.0 * fuerza).clamp(0.0, 255.0) as u8;
            anillo.material_mut().emission_color =
                Some(Color::new(canal(r), canal(g), canal(b), 255));
        }

        grupo.recalcular_caja(0.02);
    }
}

pub fn actualizar_escena(
    objetos: &mut [Box<dyn RayIntersect + Send + Sync>],
    luces: &mut [Light],
    escena: &EscenaViva,
    params: &SceneParams,
) {
    actualizar_estelas(objetos, escena, params);
    actualizar_anillos(objetos, escena, params);

    // Las hadas: caen despacio, se deshacen con el arpa y renacen, y
    // brillan con la cancion. La presencia entra en el radio Y en la
    // emision: con el radio en cero al deshacerse, no queda ni una bolita
    // palida sin brillo que delate el salto de abajo a arriba.
    //
    // Y ORBITAN: cada anillo gira entero alrededor de la Triforce (el eje
    // Y por el centro de la fuente), lentisimo, y los anillos alternan el
    // sentido para que no parezca un carrusel. El giro se aplica al lugar
    // de NACIMIENTO antes de la caida y la deriva, asi que todo lo demas
    // (deshacerse, renacer arriba) sigue igual, solo que arriba ya no es
    // el mismo punto. Como el eje pasa por el centro de cada anillo, la
    // esfera acotante del grupo sigue conteniendolo.
    //
    // Encima, un vaiven vertical AL TEMPO, chiquito y con fase propia por
    // hada, que crece con la energia lenta del tema.
    let total: usize = escena.orbes.iter().map(|(_, bases)| bases.len()).sum();
    let mut k = 0;
    for (anillo, (indice, bases)) in escena.orbes.iter().enumerate() {
        let Some(objeto) = objetos.get_mut(*indice) else {
            continue;
        };
        let Some(grupo) = (objeto.as_mut() as &mut dyn Any).downcast_mut::<GrupoAcotado>() else {
            continue;
        };

        let sentido = if anillo % 2 == 0 { 1.0 } else { -1.0 };
        let (sg, cg) = (params.giro_hadas * sentido).sin_cos();

        for (hijo, (centro, radio, emision)) in grupo.children_mut().iter_mut().zip(bases.iter()) {
            if let Some(hada) = (hijo.as_mut() as &mut dyn Any).downcast_mut::<Sphere>() {
                let girado = Vec3::new(centro.x * cg + centro.z * sg, centro.y, -centro.x * sg + centro.z * cg);
                let (posicion, presencia) =
                    hada_en(girado, k, total, params.tiempo, &params.ataques);
                let fase_vaiven = params.tiempo / (params.beat_period.max(0.05) * 4.0)
                    * 2.0
                    * std::f32::consts::PI
                    + k as f32 * 0.9;
                let vaiven = fase_vaiven.sin() * HADA_VAIVEN * (0.4 + params.energia_suave * 0.6);

                // SU NOTA. Cuando suena, el hada sube un poco, engorda un
                // poco y brilla bastante; cuando la nota se apaga, vuelve.
                // Como la envolvente de las notas es la de una cuerda
                // pulsada (entra de golpe, se va en un segundo largo), lo
                // que se ve es el arpegio recorriendo la fuente y dejando
                // rastro, no doce lamparitas prendiendo y apagando.
                let nota = params.notas[nota_de(k)];

                hada.center = posicion + Vec3::new(0.0, vaiven + nota * HADA_NOTA_SUBE, 0.0);
                hada.radius = radio * presencia * (1.0 + nota * HADA_NOTA_TAMANIO);
                hada.material.emission_color = Some(escalar(
                    *emision,
                    // El brillo de la cancion es el PISO y la nota es lo
                    // que se le suma. Asi ninguna hada se apaga del todo
                    // (la fuente tiene que estar siempre viva) pero la que
                    // tiene la nota se destaca sobre las demas.
                    (params.orb_emission * (1.0 + nota * HADA_NOTA_BRILLO)).min(1.0) * presencia,
                ));
            }
            k += 1;
        }
    }

    // El agua: los anillos viajan desde el pedestal (la fase avanza con el
    // tiempo) y su altura la pone el bajo de la cancion; encima, las
    // causticas de la textura se deslizan despacio para que el fondo de la
    // piscina no se vea congelado. Todo en funcion de `tiempo`, sin
    // acumular: se dibuje a 5 o a 40 cuadros por segundo, el agua esta en
    // el mismo lugar en el mismo segundo.
    if let Some((grupo, hijo)) = escena.agua {
        let agua = objetos
            .get_mut(grupo)
            .and_then(|o| (o.as_mut() as &mut dyn Any).downcast_mut::<GrupoAcotado>())
            .and_then(|g| g.children_mut().get_mut(hijo))
            .and_then(|h| (h.as_mut() as &mut dyn Any).downcast_mut::<Plane>());
        if let Some(agua) = agua {
            agua.ripple_strength = params.oleaje;
            agua.ripple_phase = params.tiempo * AGUA_VELOCIDAD;
            agua.material
                .texture
                .set_uv_offset(params.tiempo * 0.012, params.tiempo * 0.008);
            // El agua RESPIRA con el tema: su brillo propio sigue el sobre
            // lento de la energia, entre el 60% y el 120% del de reposo.
            if let Some(base) = escena.agua_emision {
                agua.material.emission_color =
                    Some(escalar(base, 0.6 + params.energia_suave * 0.6));
            }
        }
    }

    // La Triforce late con el ritmo: cada ataque y cada beat la encienden
    // por encima de su brillo de reposo, y entre golpes se queda en un 70%.
    if let Some((grupo, hijos)) = &escena.triforce {
        let g = objetos
            .get_mut(*grupo)
            .and_then(|o| (o.as_mut() as &mut dyn Any).downcast_mut::<GrupoAcotado>());
        if let Some(g) = g {
            let factor = 0.7 + params.pulso * 0.6;
            for (hijo, base) in hijos {
                if let Some(t) = g
                    .children_mut()
                    .get_mut(*hijo)
                    .and_then(|h| (h.as_mut() as &mut dyn Any).downcast_mut::<Triangle>())
                {
                    t.material.emission_color = Some(escalar(*base, factor));
                }
            }
        }
    }

    // El polvo de hada deriva: cada mota sube y baja y se mece con senos
    // de fase propia, lento, como pelusa en el aire. Funcion pura del
    // tiempo: nunca se aleja mas de `POLVO_ALCANCE` de donde nacio.
    for (grupo, centros) in &escena.polvo {
        let g = objetos
            .get_mut(*grupo)
            .and_then(|o| (o.as_mut() as &mut dyn Any).downcast_mut::<GrupoAcotado>());
        let Some(g) = g else { continue };
        for (i, (hijo, centro)) in g.children_mut().iter_mut().zip(centros.iter()).enumerate() {
            if let Some(mota) = (hijo.as_mut() as &mut dyn Any).downcast_mut::<Sphere>() {
                let fase = i as f32 * 1.7;
                let t = params.tiempo;
                mota.center = centro
                    + Vec3::new(
                        (t * 0.15 + fase).sin() * 0.25,
                        (t * 0.11 + fase * 0.5).sin() * 0.4,
                        (t * 0.13 + fase * 1.3).cos() * 0.25,
                    );
            }
        }
    }

    // LAS RUPIAS giran sobre su eje y flotan. Girar un triangulo es
    // reescribir sus tres vertices: no hay matriz de transformacion en
    // este trazador, la geometria son las coordenadas y punto. Se rota
    // alrededor del eje vertical que pasa por el centro de la rupia, asi
    // que la esfera acotante del grupo no cambia de tamanio y solo hay que
    // darle margen para el flote.
    for rupia in &escena.rupias {
        let Some(grupo) = objetos
            .get_mut(rupia.grupo)
            .and_then(|o| (o.as_mut() as &mut dyn Any).downcast_mut::<GrupoAcotado>())
        else {
            continue;
        };

        // El tema las apura, pero como un CORRIMIENTO DE FASE y no
        // dividiendo el periodo.
        //
        // La tentacion es escribir `tiempo / vuelta(energia)`, y esta mal:
        // cuando la energia cambia, cambia el divisor de un `tiempo` que
        // ya vale ciento cuarenta, y la rupia salta a otra orientacion de
        // un cuadro al siguiente. Sumando la energia como fase, el angulo
        // es continuo por construccion (la energia es un promedio de dos
        // segundos y medio, asi que se mueve despacio) y lo que se ve es
        // que giran mas rapido mientras el tema crece.
        let angulo = params.tiempo * 2.0 * std::f32::consts::PI / RUPIA_VUELTA
            + params.energia_suave * RUPIA_APURO
            + rupia.fase;
        let (sa, ca) = angulo.sin_cos();
        let alto = (params.tiempo * 0.6 + rupia.fase).sin() * RUPIA_FLOTE;
        let girar = |v: &Vec3| {
            Vec3::new(
                rupia.centro.x + v.x * ca + v.z * sa,
                rupia.centro.y + v.y + alto,
                rupia.centro.z - v.x * sa + v.z * ca,
            )
        };

        for (hijo, (a, b, c)) in grupo.children_mut().iter_mut().zip(rupia.caras.iter()) {
            if let Some(t) = (hijo.as_mut() as &mut dyn Any).downcast_mut::<Triangle>() {
                t.a = girar(a);
                t.b = girar(b);
                t.c = girar(c);
            }
        }
    }

    // LOS CRISTALES respiran con la armonia. Cada uno tiene su tercio del
    // circulo de quintas y su propio color, asi que cuando el tema se
    // mueve de una familia de acordes a otra, la luz cambia de esquina de
    // la plaza. Van con la envolvente larga (`notas_lentas`): las hadas
    // marcan la melodia nota a nota y los cristales, mucho mas lento, el
    // acorde de fondo.
    for (grupo, hijo, color, region) in &escena.cristales {
        let luz = objetos
            .get_mut(*grupo)
            .and_then(|o| (o.as_mut() as &mut dyn Any).downcast_mut::<GrupoAcotado>())
            .and_then(|g| g.children_mut().get_mut(*hijo))
            .and_then(|h| (h.as_mut() as &mut dyn Any).downcast_mut::<Sphere>());

        if let Some(luz) = luz {
            let energia = params.armonia[*region];
            luz.material.emission_color =
                Some(escalar(*color, CRISTAL_PISO + energia * CRISTAL_RANGO));
        }
    }

    // Las luces: intensidad original por el multiplicador de la cancion.
    // En el silencio la fuente se queda con la cyan cenital y un resto de
    // las otras; con el tema arriba, las de color crecen y la fuente
    // entera cambia de temperatura sin tocar un solo objeto.
    for (i, luz) in luces.iter_mut().enumerate() {
        let Some(base) = escena.luces_base.get(i) else {
            continue;
        };
        let Some(multiplicador) = params.light_multipliers.get(i) else {
            continue;
        };

        luz.intensity = base * multiplicador;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOTAL: usize = 23;

    /// Un arpa que toca una nota cada 1.3 segundos durante `hasta`.
    fn arpa(hasta: f32) -> Vec<(usize, f32)> {
        (0..)
            .map(|j| (j, j as f32 * 1.3))
            .take_while(|(_, t)| *t < hasta)
            .collect()
    }

    /// La caida es continua y lenta: entre dos cuadros seguidos el hada se
    /// mueve un pelo, nunca salta. El unico momento en que la posicion SI
    /// salta (de abajo a arriba, al renacer) pasa con presencia cero.
    #[test]
    fn las_hadas_caen_sin_saltos() {
        let base = Vec3::new(1.0, 2.0, 0.5);
        let paso = 1.0 / 30.0;
        let ataques = arpa(200.0);

        for k in [0, 3, 22] {
            let mut t = 0.0;
            while t < 120.0 {
                let (a, pa) = hada_en(base, k, TOTAL, t, &ataques);
                let (b, pb) = hada_en(base, k, TOTAL, t + paso, &ataques);

                let salto = (b - a).magnitude();
                if salto > 0.05 {
                    assert!(
                        pa < 0.05 && pb < 0.05,
                        "salto de {salto} con presencia {pa}/{pb} en t = {t}"
                    );
                }
                assert!((0.0..=1.0).contains(&pa));
                t += paso;
            }
        }
    }

    /// Cuando le toca su nota, el hada se deshace (presencia a cero en medio
    /// segundo) y renace arriba, en su lugar de nacimiento.
    #[test]
    fn las_hadas_se_deshacen_con_el_arpa() {
        let base = Vec3::new(0.0, 2.0, 0.0);
        let ataques = arpa(200.0);

        // El ataque j le toca a la hada (j * 7) % 23: el ataque 1 a la 7.
        let k = 7;
        let nota = 1.3;

        let (_, antes) = hada_en(base, k, TOTAL, nota - 0.1, &ataques);
        let (_, deshecha) = hada_en(base, k, TOTAL, nota + HADA_DISOLUCION, &ataques);
        let (arriba, naciendo) = hada_en(base, k, TOTAL, nota + HADA_DISOLUCION + 0.3, &ataques);

        assert!(antes > 0.99);
        assert!(deshecha < 1e-3);
        assert!(naciendo > 0.0 && naciendo < 1.0);
        assert!((arriba.y - base.y).abs() < 0.05, "renace arriba, no en y = {}", arriba.y);
    }

    /// Doce hadas seguidas se quedan con las doce notas, sin repetir: es
    /// lo que hace el circulo de quintas, y es lo que garantiza que el
    /// arpegio se vea recorrer la fuente en vez de encender siempre a las
    /// mismas.
    #[test]
    fn cada_hada_tiene_su_nota() {
        let mut vistas: Vec<usize> = (0..12).map(nota_de).collect();
        vistas.sort();
        assert_eq!(vistas, (0..12).collect::<Vec<_>>());

        // Y dos hadas vecinas estan a una quinta, no a un semitono: las
        // notas que chocan al oido caen lejos en la fuente.
        for k in 0..23 {
            let salto = (nota_de(k + 1) + 12 - nota_de(k)) % 12;
            assert_eq!(salto, 7, "el hada {k} y la siguiente no estan a una quinta");
        }
    }

    /// Nunca se alejan de donde nacieron mas que el margen con el que se
    /// arman sus grupos acotados, y nunca suben por encima de donde nacen.
    #[test]
    fn las_hadas_no_se_salen_del_margen() {
        let base = Vec3::new(-0.5, 3.0, 1.0);
        let ataques = arpa(200.0);
        for k in 0..TOTAL {
            let mut t = 0.0;
            while t < 150.0 {
                let (p, _) = hada_en(base, k, TOTAL, t, &ataques);
                let d = p - base;
                assert!(d.x.abs() <= HADA_ALCANCE && d.z.abs() <= HADA_ALCANCE);
                assert!(d.y <= 0.0 && -d.y <= HADA_ALCANCE, "y = {}", d.y);
                t += 0.25;
            }
        }
    }
}
