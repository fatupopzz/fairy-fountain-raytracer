//! La sincronizacion con la musica, leida de un analisis hecho afuera.
//!
//! NO HAY KEYFRAMES. No hay una tabla escrita a mano diciendo "en el segundo
//! 63 arranca el coro". Lo que hay es `loser_sync.json`, que genera
//! `analizar_audio.py` pasandole FFT y deteccion de beats a la cancion, y de
//! ahi sale todo: cuanta energia tiene cada banda en cada instante, donde
//! pega la bateria, donde estan los beats y donde cambia de seccion.
//!
//! La diferencia practica es que la escena ya no puede desincronizarse por
//! haber medido mal: no esta interpretando la cancion, la esta LEYENDO. Y si
//! se cambia el tema, no hay que reescribir nada, hay que correr el script.
//!
//! LAS TRES BANDAS Y QUE HACEN. El reparto no es arbitrario, sigue lo que
//! cada banda ES para el que escucha:
//!
//!   - `bass` es lo que se SIENTE (el kick, el bajo). Maneja lo que golpea:
//!     el bloom, el anillo principal, las luces rosa y magenta.
//!   - `mid` es lo que se OYE (voces, sintetizadores). Maneja lo que
//!     envuelve: la niebla, el anillo interior, las luces suaves.
//!   - `high` es lo que BRILLA (hi-hat, platillos). Maneja los laseres y la
//!     cian, que son lo filoso de la escena.
//!
//! Afuera de este archivo nadie sabe que existe un JSON. El loop llama a
//! `get_scene_params(t)` y usa lo que salga.

use nalgebra_glm::Vec3;
use std::fs;

/// Cuantos haces de laser tiene la escena.
pub const LASERES: usize = 8;

/// Cuantos segundos de ataques se le pasan a la escena en cada cuadro. Lo
/// que hace falta es saber cuando se deshizo cada hada por ultima vez, y
/// con ~0.8 ataques por segundo repartidos entre veintitres hadas, a cada
/// una le toca uno cada ~30 segundos: con un minuto se cubre casi siempre.
pub const ATAQUES_VENTANA: f32 = 60.0;

/// Donde se busca el analisis, en orden. La primera que exista, gana.
pub const RUTAS_SYNC: [&str; 3] = [
    "fairy_fountain_sync.json",
    "assets/music/fairy_fountain_sync.json",
    "assets/fairy_fountain_sync.json",
];

// ============================================================
//  LOS DATOS DEL ANALISIS
// ============================================================

/// Un cuadro de analisis: la foto de la cancion en un instante.
///
/// Son 30 por segundo. Mas seria desperdicio: el ojo no separa cambios de
/// luz mas rapidos que unos 24 por segundo, y el trazador entrega entre 3 y
/// 30 cuadros, o sea que muchas veces ni siquiera llega a mostrarlos todos.
#[derive(Clone, Copy, Debug)]
pub struct SyncFrame {
    pub t: f32,
    /// Graves: kick y bajo. 0..1.
    pub bass: f32,
    /// Medios: voces y sintetizadores. 0..1.
    pub mid: f32,
    /// Agudos: hi-hat y platillos. 0..1.
    pub high: f32,
    /// Energia total. 0..1.
    pub total: f32,
    /// Si en este cuadro pega un golpe percusivo.
    pub onset: bool,
    /// Energia de cada una de las doce notas, de do a si, sin suavizar.
    pub notas: [f32; 12],
}

/// El cuadro que se usa cuando no hay analisis: todo al 50%.
///
/// La escena queda quieta pero entera, que es mucho mejor que quedar negra:
/// se puede seguir trabajando en la geometria o en los materiales sin tener
/// el JSON a mano.
const FRAME_NEUTRO: SyncFrame = SyncFrame {
    t: 0.0,
    bass: 0.5,
    mid: 0.5,
    high: 0.5,
    total: 0.5,
    onset: false,
    notas: [0.0; 12],
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TipoSeccion {
    IntroOutro,
    Verso,
    PreCoro,
    Coro,
}

impl TipoSeccion {
    /// Del texto que escribe el script. Lo que no reconoce cae en `Verso`,
    /// que es el estado intermedio: si el script gana una categoria nueva,
    /// la escena se ve razonable hasta que alguien la agregue aca.
    fn desde(texto: &str) -> TipoSeccion {
        match texto {
            "intro_outro" => TipoSeccion::IntroOutro,
            "pre_coro" => TipoSeccion::PreCoro,
            "coro" => TipoSeccion::Coro,
            _ => TipoSeccion::Verso,
        }
    }

    /// El color de la niebla de cada seccion, en 0..1 por canal.
    ///
    /// La seccion no cambia la INTENSIDAD de nada (de eso ya se ocupan las
    /// bandas cuadro a cuadro): cambia el COLOR. Es la unica capa que
    /// trabaja en la escala de la estructura, y por eso es la que puede
    /// hacer que el coro se sienta distinto del verso aunque los dos tengan
    /// la misma energia.
    ///
    /// Los cuatro son variaciones de un mismo cyan oscuro pero LUMINOSO,
    /// alrededor de (0.06, 0.12, 0.15): la niebla de la fuente es vapor
    /// que atrapa la luz del agua, y es lo que llena las sombras lejanas
    /// para que las distancias no se mueran en negro. Tine toda la escena
    /// de ese tono y es lo que unifica la paleta. La seccion solo lo
    /// aclara un poco.
    fn color_niebla(self) -> Vec3 {
        match self {
            // Lavanda con algo de teal, y mas claro a medida que el tema
            // crece: la niebla es luz de hada suspendida, no humedad de
            // cueva.
            TipoSeccion::IntroOutro => Vec3::new(0.11, 0.09, 0.19),
            TipoSeccion::Verso => Vec3::new(0.13, 0.11, 0.22),
            TipoSeccion::PreCoro => Vec3::new(0.15, 0.12, 0.24),
            TipoSeccion::Coro => Vec3::new(0.18, 0.13, 0.26),
        }
    }

    /// El tinte del post-procesado. Va de neutro a un cyan/teal apenas
    /// empujado, poco saturado: es el color de la fuente, y en el coro se
    /// le suma un poco de rosa de las hadas.
    fn tinte(self) -> Vec3 {
        match self {
            TipoSeccion::IntroOutro => Vec3::new(1.0, 0.98, 1.06),
            TipoSeccion::Verso => Vec3::new(1.02, 0.99, 1.07),
            TipoSeccion::PreCoro => Vec3::new(1.04, 1.0, 1.08),
            TipoSeccion::Coro => Vec3::new(1.08, 1.0, 1.09),
        }
    }

    /// Las dos luces teal de los costados. Son las que dan estructura: en
    /// la intro apenas se insinuan las columnas, y a medida que la cancion
    /// crece la fuente entera se enciende.
    fn luz_teal(self) -> f32 {
        match self {
            TipoSeccion::IntroOutro => 0.5,
            TipoSeccion::Verso => 0.7,
            TipoSeccion::PreCoro => 0.85,
            TipoSeccion::Coro => 1.0,
        }
    }

    /// A que altura mira la camara.
    ///
    /// No esta en el analisis: es la unica decision de puesta en escena que
    /// queda de este lado, y cuelga de la seccion porque es lo que le da
    /// sentido. En la intro la camara mira un poco mas arriba, al techo y
    /// las columnas; apenas entra la cancion, baja a la Triforce y al agua.
    fn mira_y(self) -> f32 {
        match self {
            TipoSeccion::IntroOutro => 2.8,
            TipoSeccion::Verso => 2.2,
            _ => 2.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Seccion {
    pub t: f32,
    pub tipo: TipoSeccion,
    pub energia: f32,
}

/// El analisis entero.
pub struct SyncData {
    pub bpm: f32,
    pub duracion: f32,
    pub fps: usize,
    pub frames: Vec<SyncFrame>,
    pub beats: Vec<f32>,
    pub secciones: Vec<Seccion>,
    /// Las ENVOLVENTES: cada banda del analisis pasada por un seguidor de
    /// ataque rapido y caida lenta. Se calculan UNA vez al cargar, y desde
    /// ahi son un arreglo mas que se consulta por indice.
    ///
    /// Se precalculan y no se van acumulando cuadro a cuadro porque el
    /// resto del programa depende de que el estado de la escena sea
    /// funcion PURA del segundo en el que esta: este raytracer entrega
    /// entre 20 y 40 cuadros por segundo segun lo que haya en pantalla, y
    /// un filtro que avanzara un paso por cuadro dibujado duraria distinto
    /// en los momentos pesados, que son justo los golpes fuertes.
    /// Calculadas de antemano sobre los 30 cuadros por segundo fijos del
    /// analisis, la envolvente es la misma se dibuje como se dibuje.
    golpe: Envolventes,
    cuerpo: Envolventes,
    /// El chroma tal como viene del analisis: doce notas por cuadro.
    chroma: Vec<[f32; 12]>,
    /// Que par de notas le toca a cada uno de los cuatro cristales. No es
    /// fijo: sale de lo que ESTE tema toca de verdad (ver
    /// `repartir_regiones`).
    regiones: [[usize; 2]; 4],
    /// El mismo chroma con la envolvente LARGA: la armonia, no la
    /// melodia. Con el respiran los cristales.
    chroma_lento: Vec<[f32; 12]>,
    /// Y el mismo, pasado por el seguidor de envolvente. La caida es la
    /// de una cuerda pulsada: entra de golpe y se apaga sola en un
    /// segundo largo, que es lo que hace un arpa.
    chroma_suave: Vec<[f32; 12]>,
}

/// Las cuatro bandas del analisis, ya suavizadas.
#[derive(Default)]
pub struct Envolventes {
    bass: Vec<f32>,
    mid: Vec<f32>,
    high: Vec<f32>,
    total: Vec<f32>,
}

impl Envolventes {
    /// Valor de cada banda en el cuadro `i` del analisis.
    fn en(&self, i: usize, banda: &[f32]) -> f32 {
        banda.get(i.min(banda.len().saturating_sub(1))).copied().unwrap_or(0.5)
    }
}

/// SEGUIDOR DE ENVOLVENTE de ataque y caida distintos.
///
/// Es el filtro de un polo de siempre, `y = c * y + (1 - c) * x`, pero con
/// DOS constantes: una para cuando la señal sube y otra para cuando baja.
/// Es lo que usa cualquier compresor y cualquier medidor de volumen, y es
/// lo que le falta a esta escena para sentirse tocada en vez de medida.
///
/// Por que asimetrico: las bandas del analisis vienen de una ventana de
/// FFT cada treintavo de segundo, o sea que suben y bajan a saltos. Atadas
/// directo a una luz, la luz tiembla. Promediandolas parejo (que es lo que
/// habia), el temblor se va PERO tambien se va el golpe, porque el
/// promedio no distingue una subida brusca de una bajada. Con el ataque
/// casi instantaneo y la caida lenta, el golpe entra entero y lo que se
/// suaviza es solamente la cola: exactamente lo que hace el oido.
///
/// La constante sale de la VIDA MEDIA, que es lo unico que se puede
/// razonar en segundos: `c = exp(ln(0.5) / cuadros_de_vida_media)`. Con
/// una vida media por debajo de un cuadro de analisis (33 ms), el ataque
/// queda instantaneo, que es lo que se quiere.
fn seguir(valores: &[f32], fps: f32, ataque_ms: f32, caida_ms: f32) -> Vec<f32> {
    let coef = |ms: f32| {
        if ms <= 0.0 {
            return 0.0;
        }
        (std::f32::consts::LN_2 / -(fps * ms / 1000.0)).exp()
    };
    let (a, b) = (coef(ataque_ms), coef(caida_ms));

    let mut salida = Vec::with_capacity(valores.len());
    let mut y = valores.first().copied().unwrap_or(0.0);
    for &x in valores {
        let c = if x > y { a } else { b };
        y = c * y + (1.0 - c) * x;
        salida.push(y);
    }
    salida
}

/// Vida media del ataque, en milisegundos. Por debajo de un cuadro de
/// analisis (33 ms), asi que en la practica el golpe entra entero.
const ATAQUE_MS: f32 = 8.0;

/// Vida media de la caida del GOLPE: corta, para lo que tiene que pegar y
/// soltar (la Trifuerza, el empujon de camara, la aberracion).
const CAIDA_GOLPE_MS: f32 = 130.0;

/// Vida media de la caida del CUERPO: larga, para lo que tiene que
/// respirar con el tema (las luces, el agua, el halo).
const CAIDA_CUERPO_MS: f32 = 550.0;

/// Vida media de la caida de una NOTA. Larga, porque eso es lo que hace
/// una cuerda pulsada: entra de golpe y se apaga sola. Con una caida
/// corta las hadas parpadearian nota a nota; con esta, el arpegio deja un
/// rastro y se ve pasar por la fuente.
const CAIDA_NOTA_MS: f32 = 620.0;

/// A que potencia se eleva el perfil de notas de cada cuadro. Mas alto,
/// menos notas sobreviven: con 1 queda el chroma crudo y con 4 casi solo
/// la dominante. Ver `preparar_envolventes`.
const NITIDEZ_NOTA: f32 = 3.5;

/// Vida media de la caida de la ARMONIA. Tres veces mas larga que la de
/// una nota, porque la armonia se mueve tres veces mas lento: en este tema
/// la melodia cambia de nota cada medio compas y el acorde de fondo cada
/// dos o cuatro compases. Con la misma caida que las notas, los cristales
/// titilarian igual que las hadas y las dos capas dirian lo mismo.
const CAIDA_ARMONIA_MS: f32 = 1900.0;

/// A que potencia se eleva el perfil de regiones. Alto: con cuatro
/// casilleros nada mas, hace falta bastante para que solo quede uno.
const NITIDEZ_REGION: f32 = 5.0;

impl SyncData {
    /// Calcula las dos envolventes a partir de los cuadros ya cargados.
    fn preparar_envolventes(&mut self) {
        let fps = self.fps as f32;
        let bass: Vec<f32> = self.frames.iter().map(|f| f.bass).collect();
        let mid: Vec<f32> = self.frames.iter().map(|f| f.mid).collect();
        let high: Vec<f32> = self.frames.iter().map(|f| f.high).collect();
        let total: Vec<f32> = self.frames.iter().map(|f| f.total).collect();

        let hacer = |caida: f32| Envolventes {
            bass: seguir(&bass, fps, ATAQUE_MS, caida),
            mid: seguir(&mid, fps, ATAQUE_MS, caida),
            high: seguir(&high, fps, ATAQUE_MS, caida),
            total: seguir(&total, fps, ATAQUE_MS, caida),
        };

        self.golpe = hacer(CAIDA_GOLPE_MS);
        self.cuerpo = hacer(CAIDA_CUERPO_MS);

        // Y las doce notas, cada una con su propia envolvente: una nota
        // que se apaga no arrastra a las demas.
        if !self.chroma.is_empty() {
            // AFILAR LA NOTA DOMINANTE antes de suavizar.
            //
            // El chroma del analisis reparte energia en casi todas las
            // notas a la vez: los armonicos de una cuerda caen en otras
            // notas, la reverberacion arrastra las anteriores, y los
            // filtros del CQT se solapan. Medido sobre este tema, con el
            // chroma crudo hay ocho o nueve notas de doce por encima del
            // umbral en cualquier instante, o sea que casi todas las hadas
            // estarian encendidas siempre y no se leeria ninguna melodia.
            //
            // El arreglo es quedarse con el PERFIL y no con el valor: se
            // mide cada nota contra la mas fuerte de su cuadro, se eleva
            // esa proporcion a una potencia (lo que hunde a las
            // secundarias y deja quieta a la dominante) y se vuelve a
            // escalar por la fuerza original del cuadro, para que los
            // pasajes suaves sigan siendo suaves.
            let mut afilado = self.chroma.clone();
            for cuadro in afilado.iter_mut() {
                let maximo = cuadro.iter().copied().fold(0.0f32, f32::max);
                if maximo <= 1e-6 {
                    continue;
                }
                for v in cuadro.iter_mut() {
                    *v = (*v / maximo).powf(NITIDEZ_NOTA) * maximo;
                }
            }

            let mut suave = vec![[0.0f32; 12]; afilado.len()];
            let mut lento = vec![[0.0f32; 12]; afilado.len()];
            for n in 0..12 {
                let banda: Vec<f32> = afilado.iter().map(|c| c[n]).collect();
                for (destino, caida) in
                    [(&mut suave, CAIDA_NOTA_MS), (&mut lento, CAIDA_ARMONIA_MS)]
                {
                    let seguida = seguir(&banda, fps, ATAQUE_MS, caida);
                    for (i, v) in seguida.into_iter().enumerate() {
                        destino[i][n] = v;
                    }
                }
            }
            self.chroma_suave = suave;
            self.chroma_lento = lento;
        }
    }

    /// El orden del CIRCULO DE QUINTAS: do, sol, re, la, mi, si, fa#, do#,
    /// sol#, re#, la#, fa. Dos notas seguidas estan a una quinta.
    const QUINTAS: [usize; 12] = [0, 7, 2, 9, 4, 11, 6, 1, 8, 3, 10, 5];

    /// Reparte las doce notas entre los cuatro cristales, SEGUN LO QUE
    /// ESTE TEMA TOCA.
    ///
    /// La primera version daba tres notas fijas a cada cristal, cortando
    /// el circulo de quintas en cuatro tercios iguales. Es lo correcto en
    /// abstracto y quedaba mal en concreto: un tema vive en una tonalidad
    /// y casi no visita la zona opuesta del circulo, asi que uno de los
    /// cuatro cristales no se encendia nunca (medido sobre este tema:
    /// dominaba en 54 cuadros de 5498, contra 4248 del que mas). Una
    /// esquina de la plaza quedaba apagada toda la cancion.
    ///
    /// Asi que el reparto se calcula: se mide cuanto suena cada nota en
    /// toda la cancion, se descartan las cuatro que menos aparecen, y las
    /// OCHO que quedan se reparten de a dos. Como se ordenan por el
    /// circulo de quintas antes de emparejarlas, cada cristal sigue
    /// llevando dos notas armonicamente vecinas: no se pierde el
    /// significado, solo se deja de reservar un cristal para una zona que
    /// esta cancion no pisa.
    fn repartir_regiones(&mut self) {
        if self.chroma.is_empty() {
            return;
        }

        let cuadros = self.chroma.len() as f32;
        let mut uso: Vec<(usize, f32)> = (0..12)
            .map(|n| (n, self.chroma.iter().map(|c| c[n]).sum::<f32>() / cuadros))
            .collect();

        // Las ocho mas usadas, devueltas al orden del circulo de quintas.
        uso.sort_by(|a, b| b.1.total_cmp(&a.1));
        let mut elegidas: Vec<usize> = uso.into_iter().take(8).map(|(n, _)| n).collect();
        elegidas.sort_by_key(|n| Self::QUINTAS.iter().position(|q| q == n).unwrap_or(0));

        if elegidas.len() < 8 {
            return;
        }

        // De que forma emparejarlas. Con ocho notas puestas en circulo hay
        // exactamente DOS maneras de partirlas en cuatro parejas vecinas:
        // empezando por la primera, o corriendo el corte una posicion. Se
        // prueban las dos y gana la que reparta mas parejo, medido como la
        // diferencia entre la pareja que mas suena y la que menos.
        //
        // Sin esto el reparto sale igual de correcto pero desbalanceado:
        // con el corte fijo, un cristal dominaba el 49% de la cancion y
        // otro el 8%.
        let uso_de = |n: usize| {
            self.chroma.iter().map(|c| c[n]).sum::<f32>() / self.chroma.len() as f32
        };

        let mejor = [0usize, 1]
            .into_iter()
            .map(|corte| {
                let parejas: Vec<[usize; 2]> = (0..4)
                    .map(|r| {
                        [
                            elegidas[(corte + r * 2) % 8],
                            elegidas[(corte + r * 2 + 1) % 8],
                        ]
                    })
                    .collect();
                let pesos: Vec<f32> = parejas
                    .iter()
                    .map(|p| p.iter().map(|&n| uso_de(n)).sum::<f32>())
                    .collect();
                let desbalance = pesos.iter().copied().fold(0.0f32, f32::max)
                    - pesos.iter().copied().fold(f32::MAX, f32::min);
                (desbalance, parejas)
            })
            .min_by(|a, b| a.0.total_cmp(&b.0));

        if let Some((_, parejas)) = mejor {
            for (r, p) in parejas.into_iter().enumerate() {
                self.regiones[r] = p;
            }
        }
    }

    /// Cuanto esta sonando la familia armonica de cada cristal, de 0 a 1.
    ///
    /// La cuenta NO es "cuanta energia hay en mis notas", que fue el
    /// primer intento y no servia: con cuatro notas encendidas repartidas
    /// por el circulo, casi siempre caia alguna en cada region y los
    /// cuatro cristales quedaban prendidos en casi todos los cuadros. Lo
    /// que interesa no es si suena algo de mi familia sino si la MIA es la
    /// que manda.
    ///
    /// Por eso cada region se compara contra la mas fuerte del momento y
    /// esa proporcion se eleva a una potencia, que hunde a las secundarias
    /// y deja quieta a la dominante. Es el mismo recurso que afila la nota
    /// dominante, un nivel mas arriba. El resultado se escala por la
    /// fuerza de la region mayor, para que en los silencios no se encienda
    /// ninguna.
    fn armonia(&self, t: f32) -> [f32; 4] {
        let notas = self.notas_lentas(t);
        let suma = |r: usize| self.regiones[r].iter().map(|&n| notas[n]).sum::<f32>();

        let mayor = (0..4).map(suma).fold(0.0f32, f32::max);
        if mayor <= 1e-6 {
            return [0.0; 4];
        }

        let mut salida = [0.0f32; 4];
        for (r, v) in salida.iter_mut().enumerate() {
            *v = (suma(r) / mayor).powf(NITIDEZ_REGION) * mayor.min(1.0);
        }
        salida
    }

    /// Las doce notas con la envolvente larga: la armonia del momento.
    fn notas_lentas(&self, t: f32) -> [f32; 12] {
        if self.chroma_lento.is_empty() {
            return [0.0; 12];
        }
        let i = (t.max(0.0) * self.fps as f32) as usize;
        self.chroma_lento[i.min(self.chroma_lento.len() - 1)]
    }

    /// Las doce notas en el segundo `t`, ya suavizadas.
    fn notas(&self, t: f32) -> [f32; 12] {
        if self.chroma_suave.is_empty() {
            return [0.0; 12];
        }
        let i = (t.max(0.0) * self.fps as f32) as usize;
        self.chroma_suave[i.min(self.chroma_suave.len() - 1)]
    }

    /// Las cuatro bandas con la envolvente corta (golpe) y la larga
    /// (cuerpo) en el segundo `t`.
    fn envolvente(&self, t: f32) -> (SyncFrame, SyncFrame) {
        if self.frames.is_empty() {
            return (FRAME_NEUTRO, FRAME_NEUTRO);
        }
        let i = (t.max(0.0) * self.fps as f32) as usize;
        let arma = |e: &Envolventes| SyncFrame {
            t,
            bass: e.en(i, &e.bass),
            mid: e.en(i, &e.mid),
            high: e.en(i, &e.high),
            total: e.en(i, &e.total),
            onset: false,
            // Las notas no tienen envolvente de banda: tienen la suya,
            // con la caida de una cuerda pulsada (ver `notas`).
            notas: [0.0; 12],
        };
        (arma(&self.golpe), arma(&self.cuerpo))
    }

    /// DONDE CAE EL COMPAS: `(fase, acento)`.
    ///
    /// `fase` va de 0 en el tiempo fuerte a 1 justo antes del siguiente, y
    /// `acento` vale 1 en el primer tiempo de cada compas de cuatro y baja
    /// a 0.35 en los otros tres.
    ///
    /// La escena no tenia nada de esto: todos los beats pesaban igual, y
    /// una escena donde todos los tiempos pesan igual se siente medida y
    /// no tocada. Acentuando el uno, el ojo agarra el compas solo.
    ///
    /// El tiempo fuerte se cuenta desde el primer beat detectado, que en
    /// un tema con entrada limpia como este es el uno de verdad.
    fn compas(&self, t: f32) -> (f32, f32) {
        if self.beats.len() < 2 {
            return (0.0, 0.5);
        }
        let i = self.beats.partition_point(|&b| b <= t);
        if i == 0 {
            return (0.0, 0.5);
        }

        let desde = self.beats[i - 1];
        let hasta = self.beats.get(i).copied().unwrap_or(desde + 60.0 / self.bpm.max(1.0));
        let fase = ((t - desde) / (hasta - desde).max(1e-3)).clamp(0.0, 1.0);

        let acento = if (i - 1) % 4 == 0 { 1.0 } else { 0.35 };
        (fase, acento)
    }

    /// Cuanto falta para el proximo tiempo FUERTE, de 0 (recien paso) a 1
    /// (esta por caer).
    ///
    /// Es la ANTICIPACION, y es lo que separa una escena que responde de
    /// una que acompania: lo que solo reacciona siempre llega tarde,
    /// porque para reaccionar el golpe ya tiene que haber sonado. Con
    /// esto, el halo y la niebla se van cargando en los tres tiempos
    /// previos y sueltan en el uno.
    fn hacia_el_uno(&self, t: f32) -> f32 {
        if self.beats.len() < 2 {
            return 0.0;
        }
        let i = self.beats.partition_point(|&b| b <= t);
        if i == 0 {
            return 0.0;
        }
        // Cuantos tiempos faltan para el proximo multiplo de cuatro.
        let faltan = 4 - ((i - 1) % 4);
        let periodo = 60.0 / self.bpm.max(1.0);
        let segundos = (self.beats[i - 1] + faltan as f32 * periodo - t).max(0.0);
        let x = (1.0 - segundos / (4.0 * periodo)).clamp(0.0, 1.0);

        // AL CUBO, y no lineal. La carga se suelta de golpe en el tiempo
        // fuerte (esa es la idea: tension y descarga), pero si creciera
        // parejo durante todo el compas, la caida seria un escalon desde
        // un valor alto y se veria como un parpadeo. Al cubo, casi todo el
        // compas vale poco y la subida se concentra en el ultimo tiempo:
        // lo que se ve es un envion corto y una descarga, no un diente de
        // sierra.
        x * x * x
    }

    /// Prueba las rutas en orden y carga la primera que exista.
    ///
    /// Que no haya JSON no es motivo para no abrir el raytracer: se devuelve
    /// un analisis vacio, que responde con `FRAME_NEUTRO` a todo. La escena
    /// se ve estatica y se avisa por consola.
    pub fn cargar(rutas: &[&str]) -> SyncData {
        for ruta in rutas {
            let Ok(texto) = fs::read_to_string(ruta) else {
                continue;
            };

            match parsear(&texto) {
                Ok(datos) => {
                    println!(
                        "sync: {ruta} -> {:.1} BPM, {:.1} s, {} cuadros a {} fps, \
                         {} beats, {} secciones, {} onsets",
                        datos.bpm,
                        datos.duracion,
                        datos.frames.len(),
                        datos.fps,
                        datos.beats.len(),
                        datos.secciones.len(),
                        datos.frames.iter().filter(|f| f.onset).count()
                    );
                    const NOMBRES: [&str; 12] = [
                        "do", "do#", "re", "re#", "mi", "fa", "fa#", "sol", "sol#", "la",
                        "la#", "si",
                    ];
                    let reparto: Vec<String> = datos
                        .regiones
                        .iter()
                        .map(|p| format!("{}+{}", NOMBRES[p[0]], NOMBRES[p[1]]))
                        .collect();
                    println!("      cristales: {}", reparto.join("  "));
                    return datos;
                }
                Err(e) => eprintln!("sync: {ruta} no se pudo leer: {e}"),
            }
        }

        eprintln!(
            "sync: no aparecio el analisis en {rutas:?}. La escena corre igual, \
             pero QUIETA (todo al 50%). Para arreglarlo:\n  \
             python3 analizar_audio.py assets/music/loser.mp3 -o loser_sync.json"
        );

        SyncData {
            bpm: 120.0,
            duracion: 0.0,
            fps: 30,
            frames: Vec::new(),
            beats: Vec::new(),
            secciones: Vec::new(),
            golpe: Envolventes::default(),
            cuerpo: Envolventes::default(),
            chroma: Vec::new(),
            chroma_suave: Vec::new(),
            chroma_lento: Vec::new(),
            regiones: [[0, 1], [2, 3], [4, 5], [6, 7]],
        }
    }

    pub fn hay_analisis(&self) -> bool {
        !self.frames.is_empty()
    }

    /// El cuadro de analisis que le toca al segundo `t`. Es O(1): el indice
    /// se calcula, no se busca.
    fn frame(&self, t: f32) -> SyncFrame {
        if self.frames.is_empty() {
            return FRAME_NEUTRO;
        }

        let i = (t.max(0.0) * self.fps as f32) as usize;
        self.frames[i.min(self.frames.len() - 1)]
    }

    /// El cuadro de analisis crudo, para el modo `--sync`. La escena no lo
    /// usa: lee `get_scene_params`.
    pub fn frame_publico(&self, t: f32) -> SyncFrame {
        self.frame(if self.duracion > 0.0 { t.rem_euclid(self.duracion) } else { t.max(0.0) })
    }

    /// La seccion vigente. Son unas diez, asi que se recorre al reves y se
    /// corta en la primera que ya empezo.
    fn seccion(&self, t: f32) -> TipoSeccion {
        self.secciones
            .iter()
            .rev()
            .find(|s| s.t <= t)
            .map(|s| s.tipo)
            .unwrap_or(TipoSeccion::Verso)
    }

    /// El latigazo de la bateria: 1.0 en el golpe, cayendo despues.
    ///
    /// El guion lo describe como "multiplicar por 0.85 en cada cuadro". Aca
    /// esta escrito como funcion PURA del tiempo (`0.85` elevado a los
    /// cuadros transcurridos) y no como un estado que se actualiza en cada
    /// vuelta del render, y la diferencia importa: este raytracer entrega
    /// entre 3 y 30 cuadros por segundo segun la resolucion, asi que un
    /// decaimiento por cuadro de RENDER haria que el flash durara diez veces
    /// mas en 560x420 que en 200x150. Contado en cuadros de ANALISIS, que
    /// siempre son 30 por segundo, dura lo mismo en todas las resoluciones.
    ///
    /// Con 0.85 por cuadro el destello queda en 0.68 a los 80 ms, 0.21 a los
    /// 200 ms y deja de verse cerca de los 300. Es mas largo que los 80 ms
    /// del guion, pero es lo que da la formula del guion; se dejo la formula
    /// porque la cola es justamente lo que hace que el golpe se lea como un
    /// golpe y no como un parpadeo.
    /// Los ataques (onsets) de los ultimos `ventana` segundos hasta `t`,
    /// como `(numero de ataque desde el principio del tema, instante)`.
    ///
    /// El numero es lo que permite repartirlos entre las hadas de forma
    /// fija: el ataque 17 siempre le toca a la misma hada, se dibuje el
    /// cuadro que se dibuje. Es la pieza que hace que "las hadas se deshacen
    /// con el arpa" sea una funcion pura del tiempo y no un estado.
    fn ataques_hasta(&self, t: f32, ventana: f32) -> Vec<(usize, f32)> {
        let mut ataques = Vec::new();
        let mut numero = 0;

        for frame in &self.frames {
            if frame.t > t {
                break;
            }
            if frame.onset {
                if frame.t >= t - ventana {
                    ataques.push((numero, frame.t));
                }
                numero += 1;
            }
        }

        ataques
    }

    fn onset_flash(&self, t: f32) -> f32 {
        if self.frames.is_empty() {
            return 0.0;
        }

        let fps = self.fps as f32;
        let i = ((t.max(0.0) * fps) as usize).min(self.frames.len() - 1);

        // Mas atras que esto el destello ya vale menos de 0.01 y no hay nada
        // que mirar: 0.85^29 = 0.0086.
        const ALCANCE: usize = 30;
        let desde = i.saturating_sub(ALCANCE);

        for k in (desde..=i).rev() {
            if self.frames[k].onset {
                let cuadros = (t - self.frames[k].t) * fps;
                return 0.85f32.powf(cuadros.max(0.0));
            }
        }

        0.0
    }

    /// El respirar ritmico: 1.0 justo en el beat, cero 120 ms despues.
    ///
    /// Va aparte del flash de onset y no lo reemplaza. El onset es lo que la
    /// cancion HIZO (irregular, con silencios, con redobles); el beat es la
    /// grilla sobre la que la cancion esta escrita, que sigue existiendo
    /// aunque en ese compas no pegue nadie. Los anillos llevan los dos: el
    /// latigazo cuando hay golpe y el latido siempre.
    fn beat_pulse(&self, t: f32) -> f32 {
        let i = self.beats.partition_point(|&b| b <= t);
        if i == 0 {
            return 0.0;
        }

        let dt = t - self.beats[i - 1];
        (1.0 - dt / 0.12).max(0.0)
    }

    /// La energia total PROMEDIADA sobre los ultimos `ventana` segundos.
    ///
    /// Es la version lenta de `total`: un cuadro de analisis salta de un
    /// treintavo de segundo al siguiente, y todo lo que se ate a el
    /// parpadea. Promediando una ventana de un par de segundos queda un
    /// sobre que sube cuando el tema crece y baja cuando se calma, sin
    /// enterarse de cada nota. Es lo que hace que las hadas respiren con
    /// la cancion en vez de titilar con ella.
    ///
    /// Con la ventana en segundos y no en cuadros, el promedio no depende
    /// de los cuadros por segundo del analisis. Es funcion pura de `t`.
    fn energia_suave(&self, t: f32, ventana: f32) -> f32 {
        if self.frames.is_empty() {
            return 0.5;
        }
        let hasta = ((t.max(0.0) * self.fps as f32) as usize).min(self.frames.len() - 1);
        let cuantos = ((ventana * self.fps as f32) as usize).max(1);
        let desde = hasta.saturating_sub(cuantos);
        let n = (hasta - desde + 1) as f32;
        self.frames[desde..=hasta].iter().map(|f| f.total).sum::<f32>() / n
    }

    /// Una respiracion lenta al tempo: un seno que va de 0 a 1 cada
    /// `compases` compases de cuatro tiempos. Va en beats y no en segundos
    /// para que respire CON el tema.
    fn respiracion(&self, t: f32, compases: f32) -> f32 {
        let beat_period = 60.0 / self.bpm.max(1.0);
        let fase = t / (beat_period * 4.0 * compases) * 2.0 * std::f32::consts::PI;
        fase.sin() * 0.5 + 0.5
    }

    /// La fraccion del tema transcurrida, de 0 a 1.
    fn fraccion(&self, t: f32) -> f32 {
        if self.duracion > 0.0 {
            (t / self.duracion).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// La curva de "cuanto cine": los primeros quince segundos esta en
    /// cero (ahi la escena tiene que presentarse como un lugar, no como
    /// una pelicula) y a la mitad del tema ya llego al tope, o sea que la
    /// segunda mitad entera se ve puesta en escena.
    ///
    /// Con `smoothstep` y no lineal a proposito: lo que delata a un efecto
    /// que crece es el arranque, el momento en que aparece de la nada. La
    /// pendiente cero en las dos puntas hace que empiece y termine sin que
    /// se vea el borde.
    fn cine(&self, t: f32) -> f32 {
        let x = ((self.fraccion(t) - 0.08) / 0.42).clamp(0.0, 1.0);
        x * x * (3.0 - 2.0 * x)
    }

    /// El cielo gira UNA porcion de vuelta a lo largo del tema: el astro
    /// arranca a la izquierda de la fuente y termina a la derecha (unos 60
    /// grados en total), lo bastante lento para que no se note el
    /// movimiento y lo bastante para que no este nunca en el mismo lugar.
    ///
    /// Es el MISMO astro que sale de sol al alba y queda de luna al final
    /// (ver `luz_del_dia` y `avanzar_noche` en `main.rs`): solo cambia de
    /// color y de potencia. Que uno se ponga y la otra salga por el mismo
    /// camino no es correcto astronomicamente, pero en cuadro se lee como
    /// el paso de las horas y evita tener dos astros de los que solo uno
    /// se ve por vez.
    fn giro_cielo(&self, t: f32) -> f32 {
        -0.5 + self.fraccion(t) * 1.0
    }

    /// CUANTA LUZ DE DIA HAY, de 1 (pleno amanecer) a 0 (noche cerrada).
    ///
    /// EL ARCO VA DEL ALBA A LA NOCHE, no al reves. Antes la escena era de
    /// noche todo el tema y amanecia sobre el final, y estaba al reves de
    /// donde va la musica: el Great Fairy's Fountain Theme CRECE, el coro
    /// esta pasado el minuto y medio, y el cuadro llegaba ahi con el cielo
    /// aclarandose, o sea restandole a lo unico que tenia que quedar
    /// brillando. Yendo del alba a la noche, el clima del cuadro y el de la
    /// cancion empujan para el mismo lado: a medida que el tema se pone mas
    /// grande, el mundo alrededor se apaga y la fuente queda siendo LA
    /// UNICA cosa que ilumina. El climax cae en la oscuridad maxima.
    ///
    /// Cuatro tramos, todos unidos con `smoothstep` para que ninguno
    /// arranque con un borde visible:
    ///
    ///   0.00 - 0.12   EL ALBA, sostenida. El tema abre con el arpa sola y
    ///                 el cielo entero esta rosa y oro. La fuente casi no
    ///                 se nota todavia: no puede competir con el cielo, y
    ///                 que no compita es el punto.
    ///   0.12 - 0.58   EL DIA SE APAGA. Es el tramo largo, casi la mitad
    ///                 del tema: el resplandor se escurre del cielo, las
    ///                 estrellas aparecen, el ambiente pasa de durazno a
    ///                 violeta, y las luces de la fuente suben mientras el
    ///                 cielo baja. Es un fundido cruzado, no un apagon.
    ///   0.58 - 0.88   NOCHE CERRADA, sostenida. Cubre entero el coro y el
    ///                 climax. Aca todo lo que brilla es la fuente.
    ///   0.88 - 1.00   EL ALBA SIGUIENTE. Cierra el ciclo para que el loop
    ///                 no corte de noche a dia de un golpe: sobre la coda,
    ///                 que es lo mas quieto del tema, el horizonte se
    ///                 vuelve a encender y engancha exactamente con el
    ///                 arranque. Un dia entero por vuelta de disco.
    fn luz_del_dia(&self, t: f32) -> f32 {
        let f = self.fraccion(t);
        let suave = |x: f32| {
            let x = x.clamp(0.0, 1.0);
            x * x * (3.0 - 2.0 * x)
        };
        if f < 0.58 {
            1.0 - suave((f - 0.12) / 0.46)
        } else if f < 0.88 {
            0.0
        } else {
            suave((f - 0.88) / 0.12)
        }
    }

    // ============================================================
    //  LA FUNCION PUBLICA
    // ============================================================

    /// El estado completo de la escena en el segundo `t`.
    ///
    /// Es LA interfaz con el render loop, que no sabe que existe un JSON, ni
    /// beats, ni secciones: recibe numeros y los escribe sobre los
    /// materiales y el post-procesado.
    pub fn get_scene_params(&self, t: f32) -> SceneParams {
        // La cancion se repite, y la escena con ella. Sin esto, pasado el
        // final el indice se clava en el ultimo cuadro y la escena se queda
        // congelada en el fundido de salida mientras el tema vuelve a
        // empezar.
        let t = if self.duracion > 0.0 { t.rem_euclid(self.duracion) } else { t.max(0.0) };

        let f = self.frame(t);
        let seccion = self.seccion(t);
        let flash = self.onset_flash(t);
        let latido = self.beat_pulse(t);

        // Las dos envolventes de las bandas: `g` pega y suelta rapido, `c`
        // respira con el tema. De aca en adelante casi nada usa `f` crudo:
        // atado directo, el valor instantaneo de una banda hace temblar lo
        // que toque (ver `seguir`).
        let (g, c) = self.envolvente(t);
        let (_, acento) = self.compas(t);
        let hacia_uno = self.hacia_el_uno(t);

        // El mismo destello pero con cola larga, para el caleidoscopio y la
        // aberracion: se desvanece gradualmente, no de golpe. `flash` vale
        // 0.85^n con n en cuadros de analisis; elevarlo a ln(0.92)/ln(0.85)
        // lo convierte exactamente en 0.92^n, sin tocar el decaimiento que
        // usa el bloom, que si tiene que pegar como golpe.
        let flash_lento = flash.powf(0.513);

        // EL GOLPE DEL MOMENTO, calculado antes del struct porque ahora lo
        // usan cinco campos y no uno.
        //
        // Junta las tres cosas que pueden marcar un impacto, y las junta
        // porque NINGUNA SOLA ALCANZA EN TODO EL TEMA:
        //   - `flash`, el ataque detectado: preciso donde hay pua, pero
        //     medido sobre este tema hay tramos enteros sin ninguno (entre
        //     el segundo 120 y el 140 la musica es un swell sostenido, sin
        //     ataques: no es que el detector falle, es que no hay que
        //     detectar);
        //   - `latido`, la grilla de beats: hay 351 reparejos de punta a
        //     punta, asi que es lo unico fiable en los tramos sin ataque, y
        //     va pesado por el ACENTO del compas para que el uno pese el
        //     triple que los otros tres;
        //   - el golpe del bajo, que le da cuerpo a los dos anteriores.
        let pulso = (flash * 0.7 + latido * 0.75 * acento + g.bass * 0.25).min(1.0);


        // ---------- LAS HADAS: brillan AL RITMO ----------
        // El ritmo es lo que manda, no el volumen. Cada ataque del arpa
        // (`flash`, el onset con su caida rapida: 0.68 a los 80 ms, casi
        // nada a los 300) las enciende a full de golpe, y `flash_lento` les
        // deja una cola de un segundo para que el apagon no sea seco. El
        // piso (lo que brillan entre ataque y ataque) es BAJO a proposito y
        // apenas sube con la energia de los medios y agudos: si el piso
        // fuera alto, el destello no tendria a donde subir, que es justo lo
        // que pasaba antes con este tema. Nunca pasan de 1.0, porque su
        // emision de nacimiento ya esta al tope de los 8 bits.
        //
        // Las pendientes de energia estan escaladas a este tema, que es
        // suave: mid promedia 0.15 y high casi nunca pasa de 0.05.
        //
        // SUAVE Y LENTO, a proposito: lo que manda ya no es cada nota sino
        // el sobre de la energia de los ultimos dos segundos y medio
        // (`energia_suave`) mas una respiracion de dos compases. El ataque
        // del arpa apenas se insinua con la cola larga de `flash_lento`;
        // el golpe seco (`flash`) no entra. Las hadas suben y bajan de
        // brillo con la cancion como una marea, no como un estrobo.
        let energia = self.energia_suave(t, 2.5);
        let respira = self.respiracion(t, 2.0);
        let cine = self.cine(t);
        // LAS HADAS TAMBIEN LATEN AHORA, y es un cambio de criterio sobre
        // lo que dice el parrafo de arriba.
        //
        // Estaban atadas casi solo al sobre lento de la energia: una marea,
        // no un estrobo, que era una decision deliberada. El problema es
        // que la suma de los terminos llegaba a 1.24 contra un tope de
        // 1.00, asi que buena parte del tema las hadas estaban RECORTADAS
        // en el maximo y no se movian nada. Un objeto emisivo clavado en su
        // valor maximo no respira ni late: solo esta prendido.
        //
        // Bajando la base y metiendo el golpe, el piso deja de tocar el
        // techo y el ataque del arpa vuelve a verse en ellas, que es el
        // objeto de la escena que mas obviamente tiene que responder al
        // arpa. Sigue sin ser un estrobo: `pulso` pesa 0.30 sobre un sobre
        // lento que pesa 0.45, o sea que manda la marea y el golpe se le
        // monta encima.
        let hadas = 0.30 + energia * 0.35 + respira * 0.10 + pulso * 0.30 + cine * 0.12;
        let hadas = hadas.min(1.0);

        // ---------- HIGH: los laseres ----------
        // El chase es un pulso que viaja de haz a haz. `intervalo` es cuanto
        // tarda en pasar de uno al siguiente, y se acorta con los agudos:
        // con platillos arriba la vuelta se acelera.
        let beat_period = 60.0 / self.bpm.max(1.0);
        let intervalo = beat_period / LASERES as f32 * (1.5 - f.high);

        // Cuantos haces estan vivos. Es lo que hace que el rig CREZCA con la
        // cancion en vez de estar siempre completo: con la energia por el
        // piso se enciende uno solo y con la cancion arriba, los ocho.
        let activos = ((f.total * LASERES as f32).floor() as usize).min(LASERES);
        let emision_laser = 0.1 + f.high * 1.5;

        let mut laser_emissions = [0.0f32; LASERES];
        if activos > 0 {
            // TRES COSAS QUE EL GUION DEJABA ABIERTAS Y QUE, MEDIDAS SOBRE
            // ESTE TEMA, DEJABAN EL RIG APAGADO. Se midieron con `--sync`,
            // que informa que porcentaje del tiempo hay algun haz prendido.
            //
            // 1. El ciclo del chase recorre los haces VIVOS, no los ocho.
            //    Dandole la vuelta entera a los ocho, el pulso se pasa la
            //    mayor parte del ciclo sobre haces que estan apagados por
            //    `activos` y no se ve nada. Con `total` promediando 0.60 en
            //    esta cancion, eso son cuatro haces vivos de ocho: el rig
            //    quedaba encendido el 39% del tiempo, medido.
            //
            // 2. El destello dura AL MENOS lo que tarda el pulso en pasar al
            //    haz siguiente. El guion fija 70 ms, pero el paso a 83 BPM
            //    es de 90 a 130 ms segun los agudos: con 70 fijos quedaba un
            //    hueco negro entre haz y haz en cada paso. (El guion suponia
            //    110 BPM, donde el paso son 68 ms y no hay hueco. El tema va
            //    a 83.35: ahi esta la diferencia.)
            //
            // 3. Los haces vivos se REPARTEN por el anillo en vez de ser los
            //    primeros `activos`. Con los primeros cuatro se encendia
            //    siempre el mismo lado del escenario y los haces 6 y 7 no se
            //    prendian nunca (`total` supera 0.75 en 311 de 6688 cuadros
            //    y llega a 8 en uno solo). Repartidos, cuatro haces son una
            //    cruz que abarca el escenario entero.
            let ciclo = intervalo * activos as f32;
            let ancho = intervalo.max(0.07);

            for paso in 0..activos {
                let i = (paso * LASERES) / activos;
                let fase = (t - paso as f32 * intervalo).rem_euclid(ciclo);
                laser_emissions[i] = emision_laser * (1.0 - fase / ancho).max(0.0);
            }
        }

        // LA HORA DEL DIA, y su complemento. De aca cuelga medio cuadro:
        // el ambiente, el cielo, la niebla, el bloom y cuanto brilla la
        // fuente. Se calcula una sola vez porque lo piden cinco campos.
        let dia = self.luz_del_dia(t);
        let noche = 1.0 - dia;
        // El factor por el que se multiplican las luces de la FUENTE.
        let del_dia = 0.42 + 0.58 * noche;

        SceneParams {
            // El piso y el destello un 40% mas altos para la fuente: el
            // halo del agua tiene que estar SIEMPRE. La pendiente del bass
            // se queda: con el agua siendo una emision del tamanio de la
            // piscina, mas que esto en el golpe la revienta en blanco.
            // Piso alto: el halo esta SIEMPRE, es el vapor de la fuente.
            // El halo respira con el cuerpo del bajo, pega con el golpe, y
            // ademas SE VA CARGANDO hacia el tiempo fuerte: `hacia_uno`
            // sube durante los tres tiempos previos y suelta en el uno.
            // Esa anticipacion es lo que hace que la escena parezca ir con
            // la musica en vez de correrla de atras.
            //
            // Y TODO ESO SE ESCALA POR LA NOCHE. No es una decision de
            // gusto sino de como funciona el ojo: un halo es visible en
            // proporcion a lo oscuro que esta el resto del cuadro. Con el
            // cielo del alba encendido, el mismo halo que de noche se ve
            // magico de dia se ve como una lente sucia. Y de paso es lo que
            // hace que el arco del cuadro y el de la cancion coincidan: el
            // tema crece, la noche cae, y el resplandor de la fuente crece
            // con las dos cosas a la vez en vez de con una sola.
            // MENOS SOSTENIDO Y MAS TRANSITORIO, que es de lo que depende
            // que la escena se sienta tocada.
            //
            // La version anterior colgaba casi entera de envolventes
            // LENTAS (el bajo con 550 ms de caida, `hacia_uno`, `cine`).
            // Eso hace que el halo RESPIRE con el tema, que esta bien, pero
            // no que PEGUE: medido sobre el climax, el bloom recorria de
            // 0.93 a 2.14 a lo largo de treinta segundos y se movia apenas
            // 0.27 de un golpe al siguiente. Un cambio del 12% del recorrido
            // repartido en medio segundo no se lee como un golpe, se lee
            // como nada.
            //
            // Asi que se le saca al sostenido y se le da al transitorio: el
            // termino lento baja de 1.3 a 0.80 y entra `pulso` con 0.80. El
            // recorrido total queda parecido pero el movimiento POR GOLPE
            // pasa de 0.27 a 0.46, y la media baja de 1.36 a 1.13, que de
            // paso es parte de por que el climax ya no se inunda.
            bloom_strength: (0.42 + c.bass * 0.80 + g.bass * 0.20 + pulso * 0.80
                + hacia_uno * 0.18
                + self.cine(t) * 0.28)
                * (0.40 + 0.60 * noche),
            // No esta en el guion. Se ata al bass igual que la fuerza del
            // bloom porque las dos describen el mismo halo: si el brillo
            // crece y el radio no, el bloom se ve como un recorte duro.
            bloom_radius: 5.0 + c.bass * 9.0,
            // EL UMBRAL DEL BLOOM SE ADAPTA A LA HORA, como el ojo.
            //
            // El umbral dice que luminancia hay que pasar para florecer, y
            // es ABSOLUTO. Mientras la escena fue siempre de noche un valor
            // fijo alcanzaba, porque lo unico que lo cruzaba era lo que
            // brilla solo. Con el amanecer la luz ambiente sube tres veces
            // y de dia lo cruzaria MEDIA ESCENA —el marmol, el oro, el
            // piso mojado— envolviendo la fuente en una nube blanca.
            //
            // Subirlo con la luz del dia es lo que hace un ojo (o una
            // camara) de verdad: en un cuarto oscuro una vela deslumbra, y
            // al sol la misma vela no se ve. Lo que florece no es lo que
            // pasa un brillo fijo, es lo que sobresale del brillo AMBIENTE
            // del momento.
            //
            // EL PISO SUBIO DE 0.42 A 0.48 por un problema distinto, que
            // aparece en el climax y que no es de exposicion sino de
            // TAMANO. Un halo se ve bien cuando sale de algo chico y muy
            // brillante; de algo grande y medianamente brillante no sale un
            // halo, sale niebla. El agua de la piscina son cinco por cinco
            // unidades de superficie emisiva, y en el pico del tema cruzaba
            // entera el umbral: la piscina completa entraba a la cadena de
            // mips como una mancha y el cuadro se inundaba de blanco hasta
            // tapar la Triforce. Con 0.48 la sabana de agua se queda
            // afuera y siguen pasando las chispas de las causticas, las
            // hadas y el oro, que es lo que tiene que florecer. Medido en
            // el segundo 142, el cuadro pasa de ilegible a leerse entero
            // sin perder nada del resplandor.
            bloom_threshold: 0.48 + dia * 0.24,
            // La niebla del ALBA es mas espesa: la bruma de la manana es
            // una cosa real, se levanta con el sol y se disipa despues, y
            // aca ademas hace falta por una razon de composicion. Al
            // principio la fuente todavia no brilla, asi que lo que tiene
            // que llevarse la mirada es el cielo; una bruma que separe los
            // planos y empuje el fondo hacia atras es lo que lo consigue.
            // De noche el aire se limpia y la fuente se ve nitida contra
            // negro, que es lo contrario y es lo que corresponde ahi.
            fog_density: 0.07 + c.mid * 0.40 + hacia_uno * 0.03 + dia * 0.11,
            // Y se TINE con la hora: rosa durazno al alba, el azul de la
            // seccion de noche. La niebla es aire, y el aire toma el color
            // del cielo; si el cielo esta rosa y la niebla azul, el cuadro
            // se parte en dos y no se cree ninguno de los dos.
            fog_color: {
                seccion.color_niebla() * (1.0 - dia * 0.72)
                    + Vec3::new(0.46, 0.24, 0.26) * (dia * 0.72)
            },
            // LAS LUCES SON LO QUE MAS SE VE DE LA CANCION.
            //
            // Cambiar el brillo de un objeto se nota en ese objeto;
            // cambiar una luz cambia el cuadro ENTERO, porque se mueven a
            // la vez el difuso, el especular, los reflejos, lo que pasa el
            // umbral del bloom y hasta de donde salen los god rays. Por
            // eso el rango de aca es ancho: entre el silencio y el coro,
            // la violeta cenital y la rosa del centro se multiplican por
            // cuatro, y la fuente no cambia de brillo sino de COLOR,
            // porque cada una crece con una banda distinta del analisis.
            //
            // Los topes estan para que el trazador no entregue el cuadro
            // recortado antes de que el post-procesado pueda hacer algo
            // con el.
            // Y TODAS LAS LUCES DE LA FUENTE se escalan por la hora,
            // abajo, con `del_dia`: al alba dan un 42% y de noche el
            // 100%.
            //
            // ES LA MITAD DEL EFECTO Y ES UN FUNDIDO CRUZADO, no un
            // apagon. Mientras el cielo, el ambiente y la luz del sol
            // BAJAN, estas SUBEN, y las dos rampas se cruzan por la mitad
            // del tema. Eso hace que la fuente no cambie de brillo
            // absoluto tanto como cambia de PROTAGONISMO: al principio es
            // un detalle adentro de un paisaje iluminado y al final es la
            // unica fuente de luz que queda. Que nunca lleguen a cero es
            // deliberado: el agua de la Fairy Fountain alumbra siempre,
            // tambien de dia, solo que de dia no se nota.
            light_multipliers: [
                // 0 violeta cenital: va con el cuerpo del tema (el bass de
                //   este tema es el arpa grave) y con cada ataque; es la
                //   que enciende la fuente por el hueco del techo.
                // EL TERMINO SOSTENIDO TIENE QUE CABER DEBAJO DEL TOPE, y
                // ese fue el error que costo dos intentos encontrar.
                //
                // El tope existe para que el trazador no entregue el cuadro
                // ya recortado en blanco antes de que el post-procesado
                // pueda hacer algo con el. Pero si el termino lento SOLO ya
                // lo alcanza, el golpe que se le sume no puede mover nada,
                // y la luz se queda literalmente clavada. Se instrumento y
                // era exactamente eso: en el segundo 143, justo en el uno y
                // justo entre golpes, esta luz valia 1.900 en los dos
                // casos, porque el sostenido daba 2.04 por su cuenta.
                //
                // Con estos coeficientes hay medio punto de recorrido POR
                // GOLPE donde antes habia cero: medido sobre el climax, el
                // swing pasa de 0.235 a 0.41.
                //
                // La base (0.58) es mas alta de lo que pedirian esos
                // numeros, y es a proposito. El primer ajuste la dejo en
                // 0.42 y consiguio un swing todavia mayor, 0.44, pero
                // bajando la MEDIA del climax de 1.22 a 0.86: la escena
                // pulsaba mucho mas y al mismo tiempo estaba un tercio mas
                // apagada, o sea que se cambiaba un climax lavado por uno
                // oscuro con destellos. Como el pulso dura 120 ms sobre un
                // beat de 463, la escena pasa la mayor parte del tiempo en
                // el valor BAJO, asi que la base es la que decide como se
                // ve el tema y el golpe solo decide como se siente. Con
                // 0.58 se recupera casi toda la media (1.03) y se conserva
                // casi todo el swing (0.41).
                (0.58 + c.bass * 0.95 + g.bass * 0.30 + pulso * 0.68).min(1.9) * del_dia,
                // 1 rosa del centro: con las hadas, o sea con el arpa.
                (0.50 + c.mid * 0.65 + c.high * 1.05 + g.high * 0.50 + pulso * 0.62).min(1.9)
                    * del_dia,
                // 2 y 3 teal de los costados: la seccion pone el piso y la
                //   energia del momento las hace respirar encima.
                (seccion.luz_teal() * (0.75 + c.total * 0.6)).min(1.8) * del_dia,
                (seccion.luz_teal() * (0.75 + c.total * 0.6)).min(1.8) * del_dia,
                // 4 la luz del agua: el agua alumbra SIEMPRE, asi que
                //   tiene el piso mas alto de todas, pero sube fuerte con
                //   el cuerpo del tema.
                (0.6 + c.total * 0.85 + g.bass * 0.4).min(1.5) * del_dia,
            ],
            orb_emission: hadas,
            // El agua respira con el bajo (el arpa grave de este tema) y
            // se sacude un poco en cada ataque. El piso de 0.05 la deja
            // apenas viva en el silencio: un espejo que tiembla.
            // El agua se sacude mas: el bajo levanta la ola y cada ataque
            // le pega un golpe encima. Es de las pocas cosas que se ven
            // moverse en la escena, asi que rinde.
            oleaje: (0.05 + c.bass * 0.35 + g.bass * 0.25 + flash * 0.18).min(0.55),
            giro_cielo: self.giro_cielo(t),
            luz_del_dia: dia,
            pulso,
            energia_suave: energia,
            // Una vuelta entera cada 24 compases (unos 45 segundos a este
            // tempo): se nota que giran solo si uno se queda mirando.
            notas: self.notas(t),
            armonia: self.armonia(t),
            giro_hadas: t / (beat_period * 4.0 * 24.0) * 2.0 * std::f32::consts::PI,
            beat_period,
            cine: self.cine(t),
            tiempo: t,
            ataques: self.ataques_hasta(t, ATAQUES_VENTANA),
            laser_emissions,
            camera_target_y: seccion.mira_y(),
            color_shift: seccion.tinte(),

            // Los sectores del caleidoscopio cuelgan del bass, igual que el
            // bloom: en el golpe la imagen se parte en mas pedazos. Cuatro
            // son cuatro cuadrantes que todavia dejan leer la escena; ocho
            // ya es una mandala.
            kal_segments: (4.0 + f.bass * 4.0).clamp(4.0, 8.0),

            // El giro se mide en BEATS, no en segundos, y por eso va aca y
            // no en el loop de render. Contado en segundos el patron giraria
            // a una velocidad que no tiene nada que ver con el tema; contado
            // en beats, una vuelta entera del caleidoscopio son siempre los
            // mismos compases. Y como es funcion pura de `t`, la escena
            // sigue siendo la misma se dibuje a 5 o a 40 cuadros por
            // segundo.
            kal_rotation: t / beat_period * 0.2,

            // EL CALEIDOSCOPIO VIVE EN EL FONDO, y por eso puede estar
            // prendido siempre. El shader lo enmascara con la profundidad:
            // solo pliega la pared y el techo, y no toca la esfera, los
            // postes ni el piso cercano. Mientras plegaba el cuadro entero
            // habia que dejarlo en cero porque se comia el escenario; ahora
            // que no puede tocarlo, un fondo de mandala permanente es
            // justamente lo que le da profundidad a la escena.
            //
            // Igual sigue habiendo destello, pero apenas: la base es un
            // fantasma de simetria que se ve todo el tiempo y el golpe le
            // suma 0.15 como mucho, nada de flash obvio. El decaimiento es
            // `flash_lento`, que vale `0.92` elevado a los cuadros de
            // analisis transcurridos desde el ultimo onset. Eso es
            // exactamente "multiplicar por 0.92 en cada cuadro", pero como
            // FUNCION DE `t` y no como un acumulador del loop de render. La
            // diferencia no es de estilo: este raytracer entrega entre 5 y
            // 40 cuadros por segundo segun lo que haya en pantalla, asi que
            // un `*= 0.92` por cuadro dibujado haria que el destello durara
            // el triple en los momentos pesados, que son justo los golpes
            // fuertes. Contado en cuadros de analisis (30 por segundo,
            // fijos), dura lo mismo siempre.
            //
            // CASI APAGADO para la fuente: la mandala permanente era
            // demasiado para un lugar que tiene que sentirse en calma. Queda
            // un fantasma de simetria en el fondo solo en los ataques del
            // arpa, que se desvanece con ellos.
            kal_mix: 0.18 * flash_lento,

            // La aberracion sigue al mismo destello lento: base casi
            // imperceptible para que la lente tenga un poco de personalidad,
            // y una leve separacion de color en el golpe, hasta 0.005. Van
            // juntas a proposito, son el mismo impacto visto en dos lugares.
            chromatic_aberration: 0.0008 + 0.005 * flash_lento,

            // Constante y sutil, cinematografico. El grano es el soporte, no
            // un efecto: si respirara con la cancion se notaria como un
            // filtro y dejaria de leerse como pelicula.
            grain_amount: 0.012,
        }
    }
}

// ============================================================
//  LO QUE VE EL RENDER
// ============================================================

/// El estado de la escena en un instante, ya listo para escribir.
///
/// Los multiplicadores son sobre el valor con el que se CONSTRUYO la escena,
/// no valores absolutos: cambiarle el color o la potencia a una luz no
/// obliga a tocar nada de aca.
#[derive(Clone, Debug)]
pub struct SceneParams {
    pub bloom_strength: f32,
    pub bloom_radius: f32,
    /// La luminancia a partir de la cual un pixel alimenta el bloom. SUBE
    /// con la luz del dia: ver el comentario en `get_scene_params`.
    pub bloom_threshold: f32,
    pub fog_density: f32,
    /// Color de la niebla, por canal en 0..1.
    pub fog_color: Vec3,
    /// Un multiplicador por luz puntual, en el orden en que se construyen:
    /// 0 cyan cenital, 1 rosa del centro, 2 teal izquierda, 3 teal derecha,
    /// 4 la luz del agua.
    pub light_multipliers: [f32; 5],
    /// Multiplica la emision de las hadas, entre ~0.4 y 1.0.
    pub orb_emission: f32,
    /// Altura del oleaje del agua (la fuerza con la que se inclina la
    /// normal). Casi quieta en el silencio, y el bajo la levanta.
    pub oleaje: f32,
    /// Cuanto giro el cielo (la luna y las estrellas) desde el principio
    /// del tema, en radianes.
    pub giro_cielo: f32,
    /// Cuanta luz de dia hay, de 1 (el alba con la que abre el tema) a 0
    /// (la noche cerrada del climax). Ver `Sync::luz_del_dia`: de esto
    /// cuelgan el ambiente, el resplandor del cielo, la niebla, el bloom y
    /// cuanto brilla la fuente.
    pub luz_del_dia: f32,
    /// El golpe del momento, de 0 a 1: ataques y latido juntos. Es lo que
    /// hace pulsar a la Triforce y lo que le da a la camara su empujoncito
    /// en cada tiempo.
    pub pulso: f32,
    /// La energia del tema promediada sobre los ultimos segundos, de 0 a
    /// 1: el sobre LENTO de la cancion. Con el respiran el agua y el
    /// vaiven de las hadas.
    pub energia_suave: f32,
    /// QUE NOTA suena, de do a si: doce valores de 0 a 1, ya con la
    /// envolvente del arpa puesta (ataque instantaneo, cola larga).
    ///
    /// Es lo que le permite a la escena mostrar la MELODIA y no solo el
    /// volumen. Las tres bandas dicen cuanta energia hay y donde; un
    /// arpegio y un acorde sostenido con la misma energia en medios son,
    /// para ellas, lo mismo. Con esto cada hada se queda con una nota y se
    /// enciende cuando suena, y el arpegio se ve recorrer la fuente.
    pub notas: [f32; 12],
    /// Cuanto esta sonando la familia armonica de cada cristal, de 0 a 1.
    ///
    /// Es la otra cara de `notas`: aquellas son la MELODIA (nota a nota,
    /// cola corta) y esto la ARMONIA (de a pares de notas, cola tres veces
    /// mas larga). Las hadas llevan una y los cristales la otra, asi que
    /// las dos capas se mueven a velocidades distintas y no dicen lo
    /// mismo: adentro pasa el arpegio, afuera cambia el acorde.
    ///
    /// Que notas le tocan a cada cristal lo decide el tema (ver
    /// `repartir_regiones`).
    pub armonia: [f32; 4],
    /// Cuanto giraron los anillos de hadas alrededor de la Triforce, en
    /// radianes. Contado en beats: una vuelta cada tantos compases.
    pub giro_hadas: f32,
    /// Periodo de un beat, en segundos: para que lo que se mece lo haga al
    /// tempo.
    pub beat_period: f32,
    /// CUANTO CINE, de 0 a 1, segun lo avanzada que este la cancion.
    ///
    /// Es la unica perilla que crece de punta a punta del tema en vez de
    /// responder a la musica instante a instante, y de ella cuelga todo lo
    /// que hace que el final se vea mas puesto en escena que el principio:
    /// las bandas del formato ancho, las estelas de las luces, la
    /// profundidad de campo, cuanto se acerca la camara y cuanto brillan
    /// las hadas. La idea es que uno no note el cambio mientras pasa y si
    /// note que el final es otra cosa.
    pub cine: f32,
    /// El segundo de la cancion (ya envuelto por su duracion): la fase de
    /// la deriva de las hadas.
    pub tiempo: f32,
    /// Los ataques del arpa de los ultimos `ATAQUES_VENTANA` segundos, como
    /// `(numero de ataque, instante)`. Con cada uno se deshace un hada.
    pub ataques: Vec<(usize, f32)>,
    /// La emision de cada haz, INDIVIDUAL: el chase los enciende de a uno.
    pub laser_emissions: [f32; LASERES],
    /// A que altura mira la camara. Lo pide el loop de render, que ya tenia
    /// la camara apuntando por aca antes de que existiera el analisis.
    pub camera_target_y: f32,
    /// Tinte global del post-procesado, por canal.
    pub color_shift: Vec3,

    /// En cuantos sectores parte la imagen el caleidoscopio, de 4 a 10.
    pub kal_segments: f32,
    /// Cuanto gira el patron del caleidoscopio, en radianes.
    pub kal_rotation: f32,
    /// Cuanto del caleidoscopio se aplica: 0 deja el cuadro intacto, 1 es
    /// el efecto puro. Vale 0 casi siempre; sube solo en los golpes.
    pub kal_mix: f32,

    /// Cuanto se separan los canales de color hacia los bordes.
    pub chromatic_aberration: f32,
    /// Cuanto grano se suma encima de todo.
    pub grain_amount: f32,
}

// ============================================================
//  EL PARSER DE JSON
// ============================================================
//
// Escrito a mano y no con serde, a proposito. No es una postura: es que el
// archivo que hay que leer tiene seis claves arriba y seis por cuadro, todas
// numeros, y traer una dependencia con derive y macros para eso costaria mas
// tiempo de compilacion que lo que ocupan estas doscientas lineas.
//
// Lo que SI implementa: objetos, arrays, numeros, strings, booleanos, null y
// anidamiento arbitrario para poder saltear lo que no interesa. Lo que no:
// decodificar escapes unicode (`é`), porque en este archivo las unicas
// cadenas son los nombres de las claves y los cuatro tipos de seccion.

struct Lector<'a> {
    b: &'a [u8],
    i: usize,
}

type Resultado<T> = Result<T, String>;

impl<'a> Lector<'a> {
    fn nuevo(texto: &'a str) -> Lector<'a> {
        Lector { b: texto.as_bytes(), i: 0 }
    }

    fn error<T>(&self, que: &str) -> Resultado<T> {
        // La posicion es lo unico que hace util un error de parseo: sin ella
        // "se esperaba ," sobre un archivo de 6900 objetos no dice nada.
        Err(format!("{que} en el byte {}", self.i))
    }

    fn espacios(&mut self) {
        while self.i < self.b.len() && self.b[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }

    fn ojear(&mut self) -> Option<u8> {
        self.espacios();
        self.b.get(self.i).copied()
    }

    fn comer(&mut self, c: u8) -> Resultado<()> {
        if self.ojear() == Some(c) {
            self.i += 1;
            Ok(())
        } else {
            self.error(&format!("se esperaba '{}'", c as char))
        }
    }

    /// Consume una palabra literal (`true`, `false`, `null`).
    ///
    /// Salta los espacios primero. Sin eso, un `"onset": true` con el
    /// espacio despues de los dos puntos (que es como lo escribe cualquier
    /// serializador con sangria) no se reconoce, y el parser se cae en el
    /// unico booleano del archivo.
    fn literal(&mut self, palabra: &str) -> bool {
        self.espacios();

        if self.b[self.i..].starts_with(palabra.as_bytes()) {
            self.i += palabra.len();
            true
        } else {
            false
        }
    }

    fn numero(&mut self) -> Resultado<f32> {
        self.espacios();
        let desde = self.i;

        while self.i < self.b.len() {
            match self.b[self.i] {
                b'0'..=b'9' | b'-' | b'+' | b'.' | b'e' | b'E' => self.i += 1,
                _ => break,
            }
        }

        // Se delega el parseo real a la biblioteca estandar en vez de armar
        // el numero a mano: exponentes, signos y decimales largos ya estan
        // resueltos ahi, y mejor.
        std::str::from_utf8(&self.b[desde..self.i])
            .ok()
            .and_then(|s| s.parse::<f32>().ok())
            .map_or_else(|| self.error("numero invalido"), Ok)
    }

    fn cadena(&mut self) -> Resultado<String> {
        self.comer(b'"')?;
        let mut salida = String::new();

        while self.i < self.b.len() {
            match self.b[self.i] {
                b'"' => {
                    self.i += 1;
                    return Ok(salida);
                }
                b'\\' => {
                    // Sin decodificar: se toma el caracter siguiente tal
                    // cual. Alcanza para no cortarse en un `\"` y para que
                    // una ruta con barras no rompa el parseo.
                    self.i += 1;
                    if let Some(&c) = self.b.get(self.i) {
                        salida.push(c as char);
                        self.i += 1;
                    }
                }
                c => {
                    salida.push(c as char);
                    self.i += 1;
                }
            }
        }

        self.error("cadena sin cerrar")
    }

    /// Se come el valor que venga, sea lo que sea, y sigue.
    ///
    /// Es lo que permite que el script gane campos nuevos sin romper esto:
    /// las claves que el Rust no conoce se saltean enteras, con su
    /// anidamiento y todo.
    fn saltar_valor(&mut self) -> Resultado<()> {
        match self.ojear() {
            Some(b'"') => {
                self.cadena()?;
                Ok(())
            }
            Some(b'{') | Some(b'[') => {
                let (abre, cierra) = if self.ojear() == Some(b'{') {
                    (b'{', b'}')
                } else {
                    (b'[', b']')
                };

                let mut hondo = 0usize;
                while let Some(c) = self.ojear() {
                    if c == b'"' {
                        // Las llaves dentro de una cadena no cuentan.
                        self.cadena()?;
                        continue;
                    }

                    self.i += 1;
                    if c == abre {
                        hondo += 1;
                    } else if c == cierra {
                        hondo -= 1;
                        if hondo == 0 {
                            return Ok(());
                        }
                    }
                }

                self.error("valor sin cerrar")
            }
            Some(b't') if self.literal("true") => Ok(()),
            Some(b'f') if self.literal("false") => Ok(()),
            Some(b'n') if self.literal("null") => Ok(()),
            Some(_) => self.numero().map(|_| ()),
            None => self.error("se acabo el archivo"),
        }
    }

    /// Recorre un objeto llamando a `campo` con cada clave.
    ///
    /// El que llama decide que hacer con cada una; lo que no consuma se
    /// saltea solo. Asi cada estructura se lee en un lugar y no hay que
    /// escribir el manejo de comas dos veces.
    fn objeto(&mut self, mut campo: impl FnMut(&mut Lector<'a>, &str) -> Resultado<bool>) -> Resultado<()> {
        self.comer(b'{')?;

        if self.ojear() == Some(b'}') {
            self.i += 1;
            return Ok(());
        }

        loop {
            let clave = self.cadena()?;
            self.comer(b':')?;

            if !campo(self, &clave)? {
                self.saltar_valor()?;
            }

            match self.ojear() {
                Some(b',') => self.i += 1,
                Some(b'}') => {
                    self.i += 1;
                    return Ok(());
                }
                _ => return self.error("se esperaba ',' o '}'"),
            }
        }
    }

    /// Lo mismo para arrays: llama a `elemento` una vez por item.
    fn arreglo(&mut self, mut elemento: impl FnMut(&mut Lector<'a>) -> Resultado<()>) -> Resultado<()> {
        self.comer(b'[')?;

        if self.ojear() == Some(b']') {
            self.i += 1;
            return Ok(());
        }

        loop {
            elemento(self)?;

            match self.ojear() {
                Some(b',') => self.i += 1,
                Some(b']') => {
                    self.i += 1;
                    return Ok(());
                }
                _ => return self.error("se esperaba ',' o ']'"),
            }
        }
    }
}

fn parsear(texto: &str) -> Resultado<SyncData> {
    let mut l = Lector::nuevo(texto);

    let mut datos = SyncData {
        bpm: 120.0,
        duracion: 0.0,
        fps: 30,
        frames: Vec::new(),
        beats: Vec::new(),
        secciones: Vec::new(),
        golpe: Envolventes::default(),
        cuerpo: Envolventes::default(),
        chroma: Vec::new(),
        chroma_suave: Vec::new(),
        chroma_lento: Vec::new(),
        regiones: [[0, 1], [2, 3], [4, 5], [6, 7]],
    };

    l.objeto(|l, clave| match clave {
        "bpm" => {
            datos.bpm = l.numero()?;
            Ok(true)
        }
        "duracion" => {
            datos.duracion = l.numero()?;
            Ok(true)
        }
        "fps_analisis" => {
            datos.fps = l.numero()?.max(1.0) as usize;
            Ok(true)
        }
        "beats" => {
            l.arreglo(|l| {
                datos.beats.push(l.numero()?);
                Ok(())
            })?;
            Ok(true)
        }
        "secciones" => {
            l.arreglo(|l| {
                let mut s = Seccion { t: 0.0, tipo: TipoSeccion::Verso, energia: 0.0 };
                l.objeto(|l, k| match k {
                    "t" => {
                        s.t = l.numero()?;
                        Ok(true)
                    }
                    "tipo" => {
                        s.tipo = TipoSeccion::desde(&l.cadena()?);
                        Ok(true)
                    }
                    "energia_media" => {
                        s.energia = l.numero()?;
                        Ok(true)
                    }
                    _ => Ok(false),
                })?;
                datos.secciones.push(s);
                Ok(())
            })?;
            Ok(true)
        }
        "frames" => {
            l.arreglo(|l| {
                let mut f = SyncFrame {
                    t: 0.0,
                    bass: 0.0,
                    mid: 0.0,
                    high: 0.0,
                    total: 0.0,
                    onset: false,
                    notas: [0.0; 12],
                };
                let mut notas = [0.0f32; 12];
                l.objeto(|l, k| match k {
                    "t" => {
                        f.t = l.numero()?;
                        Ok(true)
                    }
                    "bass" => {
                        f.bass = l.numero()?;
                        Ok(true)
                    }
                    "mid" => {
                        f.mid = l.numero()?;
                        Ok(true)
                    }
                    "high" => {
                        f.high = l.numero()?;
                        Ok(true)
                    }
                    "total" => {
                        f.total = l.numero()?;
                        Ok(true)
                    }
                    "onset" => {
                        f.onset = l.literal("true");
                        if !f.onset && !l.literal("false") {
                            return l.error("se esperaba true o false");
                        }
                        Ok(true)
                    }
                    "chroma" => {
                        // Doce numeros. Si vinieran menos, los que falten
                        // quedan en cero; si vinieran mas, se ignoran: un
                        // analisis viejo sin chroma tiene que seguir
                        // cargando, solo que sin melodia.
                        let mut n = 0usize;
                        l.arreglo(|l| {
                            let v = l.numero()?;
                            if n < 12 {
                                notas[n] = v;
                            }
                            n += 1;
                            Ok(())
                        })?;
                        Ok(true)
                    }
                    _ => Ok(false),
                })?;
                f.notas = notas;
                datos.chroma.push(notas);
                datos.frames.push(f);
                Ok(())
            })?;
            Ok(true)
        }
        _ => Ok(false),
    })?;

    // Con el JSON presente pero sin cuadros, todo lo de arriba devolveria
    // `FRAME_NEUTRO` igual que si no existiera el archivo, pero sin avisar.
    // Mejor tratarlo como lo que es: un analisis que no sirve.
    if datos.frames.is_empty() {
        return Err("el analisis no trae ningun cuadro".to_string());
    }

    // Si el script no escribio la duracion, se deduce de los cuadros. Es
    // preferible a dejarla en cero, que apagaria el loop de la escena.
    if datos.duracion <= 0.0 {
        datos.duracion = datos.frames.len() as f32 / datos.fps as f32;
    }

    // Las envolventes se calculan aca, con los cuadros ya cargados: son
    // un recorrido por los 5500 cuadros del tema, una sola vez.
    datos.preparar_envolventes();
    datos.repartir_regiones();

    Ok(datos)
}

// ============================================================
//  COMPROBACIONES
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    const MUESTRA: &str = r#"{
        "bpm": 110.0,
        "duracion": 2.0,
        "fps_analisis": 30,
        "n_frames": 3,
        "beats": [0.0, 0.545, 1.09],
        "secciones": [
            {"t": 0.0, "tipo": "intro_outro", "energia_media": 0.05},
            {"t": 1.0, "tipo": "coro", "energia_media": 0.85}
        ],
        "frames": [
            {"t": 0.0, "bass": 0.0, "mid": 0.1, "high": 0.0, "total": 0.05, "onset": false},
            {"t": 0.0333, "bass": 0.9, "mid": 0.5, "high": 0.8, "total": 1.0, "onset": true},
            {"t": 0.0667, "bass": 0.2, "mid": 0.2, "high": 0.2, "total": 0.2, "onset": false}
        ]
    }"#;

    fn muestra() -> SyncData {
        parsear(MUESTRA).expect("la muestra tiene que parsear")
    }

    #[test]
    fn el_parser_lee_todos_los_campos() {
        let d = muestra();

        assert_eq!(d.bpm, 110.0);
        assert_eq!(d.fps, 30);
        assert_eq!(d.beats.len(), 3);
        assert_eq!(d.frames.len(), 3);
        assert_eq!(d.secciones.len(), 2);
        assert_eq!(d.secciones[1].tipo, TipoSeccion::Coro);
        assert!(d.frames[1].onset);
        assert!(!d.frames[2].onset);
        assert_eq!(d.frames[1].bass, 0.9);
    }

    /// Claves que el Rust no conoce (aca `n_frames`, y cualquiera que el
    /// script agregue manana) tienen que saltearse sin romper nada.
    #[test]
    fn el_parser_ignora_lo_que_no_conoce() {
        let con_extras = r#"{
            "bpm": 90.0, "n_frames": 1, "extra": {"a": [1, 2, {"b": "c"}], "d": null},
            "duracion": 1.0, "fps_analisis": 30,
            "beats": [], "secciones": [],
            "frames": [{"t": 0.0, "bass": 0.5, "mid": 0.5, "high": 0.5, "total": 0.5,
                        "onset": false, "nuevo_campo": true}]
        }"#;

        let d = parsear(con_extras).expect("los campos de mas no tienen que romper");
        assert_eq!(d.bpm, 90.0);
        assert_eq!(d.frames.len(), 1);
    }

    /// Sin archivo, la escena tiene que quedar quieta pero valida: nada en
    /// cero, nada en infinito.
    #[test]
    fn sin_analisis_la_escena_sigue_siendo_valida() {
        let vacio = SyncData::cargar(&["no_existe_este_archivo.json"]);
        assert!(!vacio.hay_analisis());

        let p = vacio.get_scene_params(12.3);
        assert!(p.bloom_strength > 0.0 && p.bloom_strength.is_finite());
        assert!(p.orb_emission > 0.0);
        assert!(p.light_multipliers.iter().all(|m| m.is_finite() && *m >= 0.0));
    }

    /// El onset tiene que pegar y despues caer. Es lo que hace que la escena
    /// golpee con la bateria.
    #[test]
    fn el_onset_pega_y_decae() {
        let d = muestra();
        let t_onset = 1.0 / 30.0;

        let en_el_golpe = d.onset_flash(t_onset);
        let despues = d.onset_flash(t_onset + 0.2);

        assert!(en_el_golpe > 0.99, "el flash tendria que arrancar en 1.0");
        assert!(despues < en_el_golpe * 0.5, "el flash no decayo");
        assert!(d.onset_flash(0.0) < 1e-6, "no hay onset antes del primero");
    }

    /// El chase enciende de a uno, no los ocho juntos, y crece con la
    /// energia total.
    #[test]
    fn el_chase_recorre_los_haces() {
        let d = muestra();

        // El cuadro 1 tiene total = 1.0, o sea los ocho haces vivos.
        let p = d.get_scene_params(1.0 / 30.0);
        let encendidos = p.laser_emissions.iter().filter(|e| **e > 0.01).count();
        assert!(encendidos >= 1, "no se encendio ningun haz");
        assert!(encendidos < LASERES, "el chase encendio los ocho a la vez");

        // El cuadro 0 tiene total = 0.05: no llega ni a un haz.
        let apagado = d.get_scene_params(0.0);
        assert!(apagado.laser_emissions.iter().all(|e| *e == 0.0));
    }

    /// La seccion es la que manda el color, y tiene que cambiar en el borde.
    #[test]
    fn la_seccion_cambia_el_color() {
        let d = muestra();

        assert_eq!(d.seccion(0.5), TipoSeccion::IntroOutro);
        assert_eq!(d.seccion(1.5), TipoSeccion::Coro);
        assert!(d.seccion(1.5).color_niebla().x > d.seccion(0.5).color_niebla().x);
    }
}

#[cfg(test)]
mod tests_envolvente {
    use super::*;

    /// El seguidor ataca rapido y suelta lento: ante un escalon sube casi
    /// entero en un cuadro, y despues de apagarse tarda mucho mas en
    /// volver a cero. Es toda la gracia del filtro asimetrico.
    #[test]
    fn ataca_rapido_y_cae_lento() {
        // Diez cuadros en silencio, un golpe, y silencio otra vez.
        let mut señal = vec![0.0f32; 10];
        señal.push(1.0);
        señal.extend(std::iter::repeat(0.0).take(30));

        let e = seguir(&señal, 30.0, ATAQUE_MS, CAIDA_CUERPO_MS);

        // En el cuadro del golpe ya llego casi arriba.
        assert!(e[10] > 0.9, "el ataque se comio el golpe: {}", e[10]);

        // Y medio segundo despues todavia queda cola.
        let medio_segundo = 10 + 15;
        assert!(
            e[medio_segundo] > 0.2,
            "la cola se fue demasiado rapido: {}",
            e[medio_segundo]
        );

        // Pero termina bajando.
        assert!(e[e.len() - 1] < e[10]);
    }

    /// La envolvente del golpe suelta antes que la del cuerpo: son las dos
    /// velocidades con las que responde la escena.
    #[test]
    fn el_golpe_suelta_antes_que_el_cuerpo() {
        let mut señal = vec![1.0f32; 3];
        señal.extend(std::iter::repeat(0.0).take(40));

        let golpe = seguir(&señal, 30.0, ATAQUE_MS, CAIDA_GOLPE_MS);
        let cuerpo = seguir(&señal, 30.0, ATAQUE_MS, CAIDA_CUERPO_MS);

        for i in 5..40 {
            assert!(
                golpe[i] <= cuerpo[i] + 1e-4,
                "en el cuadro {i} el golpe ({}) no puede estar por encima del cuerpo ({})",
                golpe[i],
                cuerpo[i]
            );
        }
    }

    /// El acento del compas cae en uno de cada cuatro tiempos, y la
    /// anticipacion crece entre tiempo fuerte y tiempo fuerte.
    #[test]
    fn el_compas_acentua_el_uno() {
        let mut d = SyncData {
            bpm: 120.0,
            duracion: 8.0,
            fps: 30,
            frames: vec![FRAME_NEUTRO; 240],
            // Un beat cada medio segundo: 120 BPM.
            beats: (0..16).map(|i| i as f32 * 0.5).collect(),
            secciones: Vec::new(),
            golpe: Envolventes::default(),
            cuerpo: Envolventes::default(),
            chroma: Vec::new(),
            chroma_suave: Vec::new(),
            chroma_lento: Vec::new(),
            regiones: [[0, 1], [2, 3], [4, 5], [6, 7]],
        };
        d.preparar_envolventes();

        // Los tiempos 0, 4, 8... son fuertes; los demas, debiles.
        for i in 0..12 {
            let t = i as f32 * 0.5 + 0.05;
            let (_, acento) = d.compas(t);
            if i % 4 == 0 {
                assert_eq!(acento, 1.0, "el tiempo {i} tenia que ser fuerte");
            } else {
                assert!(acento < 0.5, "el tiempo {i} tenia que ser debil");
            }
        }

        // La anticipacion sube dentro del compas y se reinicia en el uno.
        let antes = d.hacia_el_uno(1.4);
        let justo_despues = d.hacia_el_uno(2.05);
        assert!(antes > justo_despues, "{antes} tenia que ser mayor que {justo_despues}");
    }
}
