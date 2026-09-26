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

use crate::grupo_acotado::GrupoAcotado;
use crate::light::Light;
use crate::plane::Plane;
use crate::ray_intersect::RayIntersect;
use crate::sphere::Sphere;
use crate::sync::SceneParams;
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
const AGUA_VELOCIDAD: f32 = 1.5;

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
pub fn actualizar_escena(
    objetos: &mut [Box<dyn RayIntersect + Send + Sync>],
    luces: &mut [Light],
    escena: &EscenaViva,
    params: &SceneParams,
) {
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
