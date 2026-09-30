//! El cielo nocturno que se ve por encima de la fuente.
//!
//! Antes el fondo era un degrade de tres azules: correcto, pero un rayo que
//! se iba al cielo devolvia un color plano y la mitad de arriba del cuadro
//! se leia como una pared pintada. Ahora es un SKYBOX de verdad: una imagen
//! equirectangular (longitud en U, latitud en V) que se genera UNA vez al
//! arrancar, por codigo, y se muestrea con la direccion del rayo. Ningun
//! archivo: el cielo es parte del programa, igual que las texturas del
//! relieve.
//!
//! Tres capas, de atras hacia adelante:
//!   1. un degrade de indigo, mas claro cerca del horizonte;
//!   2. una nebulosa de ruido fractal en dos colores (magenta y teal, la
//!      paleta de la fuente), concentrada en una banda inclinada, como la
//!      Via Lactea;
//!   3. estrellas: un hash por texel decide si hay una, otro cuanto brilla
//!      y de que color, y las mas brillantes llevan un halo de 3 x 3.
//!
//! Y encima una luna: un disco con borde suave en una direccion fija, que
//! es lo que hace que el cielo tenga un punto de luz al que mirar y de
//! donde bajan los god rays.

use crate::texture::TextureImage;
use crate::vec3::{cross, dot, normalize, Vec3};
use raylib::prelude::Color;
use std::f32::consts::PI;

/// Resolucion de la imagen equirectangular. 1024 x 512 alcanza: el cielo es
/// blando, y a 400 x 300 de trazado cada estrella ocupa un pixel de todos
/// modos.
const ANCHO: usize = 1024;
const ALTO: usize = 512;

/// Hacia donde esta la luna (se normaliza al usarla): detras de la fuente
/// (-Z), un poco a la izquierda y BAJA, a unos 11 grados sobre el
/// horizonte. Tiene que ser baja: la camara mira un poco hacia abajo y el
/// borde de arriba del cuadro queda a unos 8 grados de elevacion, asi que
/// una luna alta no aparece nunca. A 11 grados asoma justo por encima del
/// borde de roca del fondo y del techo de la fuente.
pub const LUNA_DIRECCION: Vec3 = Vec3::new(-0.28, 0.19, -0.94);

/// EL SOL de los amaneceres (se normaliza al usarlo), en el cielo sin girar:
/// pegado al mar de nubes y justo donde mira la camara en los planos
/// abiertos del principio (hacia los 15 s) y del final (hacia los 178):
/// azimut 3.1 en el cielo que gira, que con el giro de la noche y la vuelta
/// de la camara cae cerca del centro del cuadro en los dos amaneceres.
pub const SOL_DIRECCION: Vec3 = Vec3::new(-0.998, -0.02, 0.042);

/// Suma de los tres canales a partir de la cual un texel del cielo se
/// considera una estrella. La nebulosa mas encendida no pasa de 260 y el
/// degrade de fondo anda por 60; una estrella arranca en 380.
const TITILEO_UMBRAL: u32 = 330;

/// Radianes por segundo del titileo. Lento: 0.9 es un ciclo cada siete
/// segundos, y con la fase repartida por celda el cielo entero respira sin
/// que se vea un patron.
const TITILEO_VELOCIDAD: f32 = 0.9;

/// Radio angular de la luna, en radianes (~2.5 grados: el doble de la real,
/// que en un cuadro de 45 grados de campo seria un punto).
const LUNA_RADIO: f32 = 0.045;

/// HASTA DONDE SUBE LA AURORA, en radianes: unos doce grados.
///
/// El numero sale de DONDE SE VE EL CIELO, que en esta escena es una franja
/// angosta: la camara mira un poco hacia abajo, el borde de arriba del
/// cuadro queda a unos ocho grados de elevacion (por eso la luna esta a
/// once, ver `LUNA_DIRECCION`) y por abajo el borde de roca y el techo de
/// la fuente tapan el horizonte. O sea que lo que se ve del cielo va de
/// unos tres grados a unos ocho.
///
/// La primera version tenia la cortina hasta los cinco grados, con lo mas
/// encendido pegado al suelo. En cuadro no se veia: la parte brillante
/// quedaba detras de la roca y lo que asomaba por arriba era el borde
/// donde la cortina ya se esta apagando, o sea un velo verdoso sin forma.
/// Subida a doce grados, con el lomo a los cuatro, lo que entra en la
/// franja visible es el CUERPO.
const AURORA_ALTO: f32 = 1.05;

/// Cuanto ondula el borde de arriba de la cortina, en radianes.
const AURORA_ONDA: f32 = 0.07;


/// Las olas de la aurora: cuanto viven, a que velocidad corren (radianes de
/// azimut por segundo) y cuanto miden de ancho.
const OLA_VIDA: f32 = 2.4;
const OLA_VELOCIDAD: f32 = 0.9;
const OLA_ANCHO: f32 = 0.24;

/// Las cuatro paletas de la aurora, una por familia armonica: (filo, cuerpo,
/// cima). La primera es la clasica (verde, turquesa, magenta).
const PALETAS_AURORA: [[(f32, f32, f32); 3]; 4] = [
    [(70.0, 255.0, 140.0), (40.0, 210.0, 200.0), (210.0, 90.0, 255.0)],
    [(60.0, 220.0, 255.0), (80.0, 150.0, 255.0), (190.0, 110.0, 255.0)],
    [(255.0, 110.0, 190.0), (220.0, 90.0, 255.0), (140.0, 90.0, 255.0)],
    [(190.0, 255.0, 110.0), (90.0, 230.0, 150.0), (255.0, 120.0, 170.0)],
];

/// Cuanto dura una estrella fugaz, en segundos.
const ESTRELLA_VIDA: f32 = 1.9;

/// Largo de la estela, en radianes de arco recorrido.
const ESTRELLA_ESTELA: f32 = 0.30;

/// Cuanto arco recorre la cabeza en toda su vida, en radianes.
///
/// Medio radiano son unos treinta grados, algo mas de la mitad del ancho
/// del cuadro: la estrella lo cruza sin salirse por el costado a mitad de
/// camino.
const ESTRELLA_CARRERA: f32 = 0.52;

/// El grosor de la estela, en radianes. Es lo que la hace una RAYA y no
/// un trazo grueso: cero coma cero dos radianes es poco mas de un grado.
const ESTRELLA_GROSOR: f32 = 0.018;

pub struct Cielo {
    imagen: TextureImage,
    /// Cuanto giro el cielo alrededor del eje vertical, en radianes. La
    /// noche AVANZA: la luna y las estrellas cruzan el cielo a lo largo de
    /// la cancion. Lo pone `ajustar` en cada cuadro.
    giro: f32,
    /// El segundo en el que estamos: la fase del titileo de las estrellas.
    tiempo: f32,
    /// Cuanto amanecio, de 0 (noche cerrada) a 1 (el horizonte encendido).
    /// Sobre el final del tema el cielo se calienta desde abajo: un
    /// resplandor rosa y oro que sube desde el horizonte y apaga las
    /// estrellas mas bajas.
    amanecer: f32,
    /// LAS ESTRELLAS FUGACES VIVAS en este cuadro, como (indice del ataque
    /// del arpa que la lanzo, cuantos segundos hace que la lanzo).
    ///
    /// No guardan trayectoria porque no hace falta: TODO lo que describe a
    /// una estrella sale de su indice, pasado por un hash. Por donde
    /// entra, hacia donde va, cuanto corre y cuanto dura son funcion del
    /// numero del ataque, y donde esta AHORA es funcion de su edad.
    ///
    /// Eso es a proposito y es el mismo criterio que rige todo lo que se
    /// mueve con la cancion en esta escena: el estado de la escena es una
    /// FUNCION DEL SEGUNDO en el que estamos, no un acumulador del bucle
    /// de dibujado. Este trazador entrega entre quince y veintidos cuadros
    /// por segundo segun lo que haya en pantalla; con estrellas que se
    /// movieran "un poco en cada cuadro", las mismas notas darian
    /// trayectorias distintas segun lo cargada que estuviera la escena, y
    /// ademas irian mas lentas justamente en los momentos mas intensos.
    estrellas: Vec<(u32, f32)>,
    /// EL "AAAAA", de 0 a 1: cuanto esta la cancion sostenida en vez de
    /// tocada. Es lo unico que enciende la aurora. Ver `SyncData::swell`.
    swell: f32,
    /// El golpe del momento (`SceneParams::pulso`): la aurora se enciende
    /// un poco mas en cada tiempo fuerte. Lo pone `musica`.
    pulso: f32,
    /// LAS OLAS DE LA AURORA: cada ataque del arpa manda una ola de brillo
    /// que corre por la cortina hacia los dos lados desde un punto del
    /// cielo. Se guardan como (azimut de salida, edad en segundos).
    olas: Vec<(f32, f32)>,
    /// Los tres colores de la cortina (filo, cuerpo, cima), mezclados en cada
    /// cuadro segun que familia de acordes suena. Ver `musica`.
    colores: [(f32, f32, f32); 3],
    /// LA CORONA, de 0 a 1: en el climax la aurora converge en rayos hacia
    /// el cenit, sobre la fuente, como una corona boreal.
    corona: f32,
    /// La fase del compas en tiempos (segundo / periodo del beat): los
    /// pulsos que suben por la cortina van a este ritmo.
    fase_beat: f32,
    /// EL MAR DE NUBES que hay debajo de la isla, horneado al arrancar: la
    /// densidad y cuanto le da la luz, por texel de una imagen
    /// equirectangular de la mitad de abajo del cielo. Van aparte de la
    /// imagen del cielo porque su COLOR no es fijo: rosa y oro al alba,
    /// indigo de luna a la noche, y verdoso cuando la aurora esta arriba.
    nubes: Vec<(f32, f32)>,
}

/// Resolucion y alcance de la imagen de las nubes: de `NUBES_TECHO` de
/// elevacion (apenas sobre el horizonte, para que el borde se funda con el
/// cielo) hasta `NUBES_PISO` (mirando bastante hacia abajo).
const NUBES_ANCHO: usize = 1024;
const NUBES_ALTO: usize = 192;
const NUBES_TECHO: f32 = 0.06;
const NUBES_PISO: f32 = -1.25;

impl Cielo {
    /// Genera el cielo entero. Tarda unas decenas de milisegundos, una vez.
    pub fn generar() -> Cielo {
        let mut pixels = Vec::with_capacity(ANCHO * ALTO);

        // Dos campos de ruido para la nebulosa, con semillas distintas, para
        // que el magenta y el teal no caigan en las mismas nubes.
        let nube_a = campo(ANCHO, ALTO, 6, 5, 11);
        let nube_b = campo(ANCHO, ALTO, 9, 5, 23);

        let luna = normalize(&LUNA_DIRECCION);

        for y in 0..ALTO {
            // Latitud: v = 0 arriba (+Y), v = 1 abajo.
            let v = (y as f32 + 0.5) / ALTO as f32;
            let elevacion = (0.5 - v) * PI; // +pi/2 arriba, -pi/2 abajo

            for x in 0..ANCHO {
                let u = (x as f32 + 0.5) / ANCHO as f32;
                let azimut = (u - 0.5) * 2.0 * PI;

                // --- 1. El degrade base ---
                // Mas claro pegado al horizonte, oscuro hacia el cenit y
                // casi negro por debajo (que apenas se ve).
                let t = (elevacion / (PI / 2.0)).clamp(-1.0, 1.0);
                let (mut r, mut g, mut b) = if t >= 0.0 {
                    mezcla((26.0, 20.0, 58.0), (8.0, 6.0, 24.0), t.powf(0.6))
                } else {
                    mezcla((26.0, 20.0, 58.0), (6.0, 5.0, 16.0), (-t).powf(0.5))
                };

                // --- 2. La nebulosa ---
                // Una banda inclinada: el peso cae con la distancia a una
                // sinusoide en el azimut, como una Via Lactea que cruza el
                // cielo en diagonal.
                let centro_banda = 0.35 * (azimut * 1.0 + 0.8).sin() + 0.15;
                let distancia_banda = (elevacion - centro_banda).abs();
                let banda = (1.0 - distancia_banda / 0.55).clamp(0.0, 1.0).powf(1.5);

                let na = nube_a[y * ANCHO + x];
                let nb = nube_b[y * ANCHO + x];
                // Contraste: el ruido promedia 0.5, se quiere que las
                // nubes sean islas y no un velo parejo.
                let magenta = ((na - 0.45) * 2.6).clamp(0.0, 1.0) * banda;
                let teal = ((nb - 0.5) * 2.8).clamp(0.0, 1.0) * banda;

                r += magenta * 95.0 + teal * 10.0;
                g += magenta * 22.0 + teal * 70.0;
                b += magenta * 100.0 + teal * 90.0;

                // --- 3. Las estrellas ---
                // Un hash por texel: una estrella cada ~140 texeles, mas
                // densas dentro de la banda. El brillo va con una potencia
                // alta para que casi todas sean tenues y unas pocas quemen.
                let h = hash2(x as u32, y as u32);
                let umbral = 0.9915 - banda * 0.005;
                if h > umbral {
                    let brillo = hash2(x as u32 + 977, y as u32 + 131).powf(3.0);
                    let tono = hash2(x as u32 + 31, y as u32 + 7);
                    // Blancas, algunas rosas y algunas cyan.
                    let (sr, sg, sb) = if tono < 0.6 {
                        (1.0, 0.97, 0.92)
                    } else if tono < 0.8 {
                        (1.0, 0.75, 0.9)
                    } else {
                        (0.7, 0.95, 1.0)
                    };
                    let e = 120.0 + brillo * 135.0;
                    r += sr * e;
                    g += sg * e;
                    b += sb * e;
                }

                // --- 4. La luna ---
                // Distancia angular entre este texel y la direccion de la
                // luna. Adentro del radio, un disco casi blanco con un
                // borde suave; alrededor, un halo tenue de tres radios.
                let dir = direccion(azimut, elevacion);
                let cos_ang = dir.dot(&luna).clamp(-1.0, 1.0);
                let ang = cos_ang.acos();
                if ang < LUNA_RADIO * 4.0 {
                    let disco = suave(1.0 - (ang - LUNA_RADIO * 0.85) / (LUNA_RADIO * 0.15));
                    let halo = (1.0 - ang / (LUNA_RADIO * 4.0)).clamp(0.0, 1.0).powf(2.5) * 0.35;
                    let luz = disco + halo;
                    r += luz * 235.0;
                    g += luz * 225.0;
                    b += luz * 205.0;
                }

                pixels.push(Color::new(
                    r.clamp(0.0, 255.0) as u8,
                    g.clamp(0.0, 255.0) as u8,
                    b.clamp(0.0, 255.0) as u8,
                    255,
                ));
            }
        }

        Cielo {
            imagen: TextureImage {
                width: ANCHO,
                height: ALTO,
                pixels,
            },
            giro: 0.0,
            tiempo: 0.0,
            amanecer: 0.0,
            estrellas: Vec::new(),
            swell: 0.0,
            pulso: 0.0,
            olas: Vec::new(),
            colores: PALETAS_AURORA[0],
            corona: 0.0,
            fase_beat: 0.0,
            nubes: hornear_nubes(),
        }
    }

    /// Lo que la musica le pide al cielo ademas de la hora: el golpe, los
    /// ataques del arpa (cada uno una ola por la cortina), la armonia (el
    /// color de la aurora) y la corona del climax.
    pub fn musica(&mut self, pulso: f32, ataques: &[(usize, f32)], armonia: [f32; 4], corona: f32, fase_beat: f32) {
        self.pulso = pulso.clamp(0.0, 1.5);
        self.fase_beat = fase_beat;
        self.corona = corona.clamp(0.0, 1.0);

        self.olas.clear();
        for &(n, t0) in ataques.iter().rev() {
            let edad = self.tiempo - t0;
            if (0.0..OLA_VIDA).contains(&edad) {
                self.olas.push((hash2(n as u32, 4441) * 2.0 * PI - PI, edad));
                if self.olas.len() >= 8 {
                    break;
                }
            }
        }

        // EL COLOR SIGUE A LOS ACORDES. Cada una de las cuatro familias
        // armonicas (las mismas que tinen los cristales) tiene su paleta, y
        // la cortina mezcla las cuatro segun cuanto suene cada una. Cuando el
        // tema modula, el cielo cambia de color.
        let pesos: Vec<f32> = (0..4).map(|r| 0.15 + 2.0 * armonia[r].clamp(0.0, 1.0).powi(2)).collect();
        let total: f32 = pesos.iter().sum();
        let mut colores = [(0.0f32, 0.0f32, 0.0f32); 3];
        for (r, paleta) in PALETAS_AURORA.iter().enumerate() {
            let w = pesos[r] / total;
            for (c, p) in colores.iter_mut().zip(paleta.iter()) {
                c.0 += p.0 * w;
                c.1 += p.1 * w;
                c.2 += p.2 * w;
            }
        }
        self.colores = colores;
    }

    /// Cuanta aurora hay AHORA, de 0 a ~1.3, antes de la forma.
    ///
    /// Antes la aurora solo salia con las voces sostenidas (el swell, diecinueve
    /// segundos del tema). Ahora vive TODA LA NOCHE, tenue, y el swell la
    /// lleva a pleno; encima, cada tiempo fuerte le da un latido. Es lo que
    /// hace que el cielo tambien toque la cancion, y no solo la fuente.
    fn fuerza_aurora(&self) -> f32 {
        // Asoma ya en el atardecer, no solo de noche cerrada.
        let noche = (1.0 - self.amanecer * 1.25).clamp(0.0, 1.0);
        noche * (0.55 + 0.60 * self.swell + 0.35 * self.pulso)
    }

    /// Deja el cielo en el estado de este cuadro: girado `giro` radianes y
    /// con `amanecer` de resplandor en el horizonte. Se llama una vez por
    /// cuadro, antes de trazar.
    pub fn ajustar(
        &mut self,
        giro: f32,
        amanecer: f32,
        tiempo: f32,
        estrellas: &[(usize, f32)],
        swell: f32,
    ) {
        self.giro = giro;
        self.amanecer = amanecer.clamp(0.0, 1.0);
        self.tiempo = tiempo;
        self.swell = swell.clamp(0.0, 1.0);

        // QUE SALIDAS ESTAN VIVAS AHORA.
        //
        // CUALES son ya no se decide aca. Era "uno de cada seis ataques del
        // arpa", y ese filtro es aritmetico y no musical: no sabe de compas
        // ni de frase, y en este tema, que tiene diecinueve segundos de
        // voces sostenidas sin una sola pua, dejaba al cielo TREINTA Y SEIS
        // SEGUNDOS sin una estrella justo sobre el climax. Ahora las elige
        // `SyncData::estrellas_hasta`, que las pone en el tercer tiempo del
        // compas, y aca solo se descarta lo que ya cruzo.
        self.estrellas.clear();
        for &(numero, t_salida) in estrellas {
            let edad = tiempo - t_salida;
            if (0.0..ESTRELLA_VIDA).contains(&edad) {
                self.estrellas.push((numero as u32, edad));
            }
        }
    }

    /// LA AURORA: el cielo cantando el "aaaaa".
    ///
    /// Devuelve cuanta aurora hay en esta direccion, de 0 a 1, y a que
    /// altura de la cortina esta (0 abajo, 1 en el borde de arriba), que es
    /// lo que decide el color.
    ///
    /// POR QUE UNA AURORA Y NO MAS METEOROS. El tramo de las voces es lo
    /// mas grande del tema y era lo mas quieto de la escena. La tentacion
    /// es llenarlo de eventos, pero un evento es un ataque —aparece,
    /// golpea y se va— y ahi no hay ataques: hay UNA nota sostenida
    /// diecinueve segundos. Lo que se parece a eso es algo grande que
    /// esta y respira, no veinte cosas chicas que pasan. Por eso, ademas,
    /// mientras la aurora esta arriba el cielo lanza la mitad de estrellas
    /// fugaces (ver `SyncData::estrellas_hasta`): dos cosas grandes a la
    /// vez se tapan.
    ///
    /// LA FORMA: una cortina que sube desde el horizonte hasta
    /// `AURORA_ALTO`, con el borde de arriba ondulado por tres senos de
    /// frecuencias distintas —uno ancho que da la panza, uno medio que la
    /// quiebra y uno corto que le saca la simetria—, y estriada en
    /// vertical, que es lo que hace que se lea como una cortina y no como
    /// niebla. Todo deriva LENTO: la cortina entera se corre menos de dos
    /// grados por segundo, asi que en cuadro se ve moverse sin que se vea
    /// pasar.
    ///
    /// Es todo funcion de la direccion y del segundo, como el resto del
    /// cielo: cinco senos por rayo, y solo cuando el swell esta arriba.
    fn aurora(&self, azimut: f32, elevacion: f32) -> (f32, f32) {
        if !(0.0..AURORA_ALTO).contains(&elevacion) {
            return (0.0, 0.0);
        }
        // LOS PLIEGUES: la cortina no cuelga derecha, se dobla y se enrosca.
        // Se tuerce el azimut segun la altura, y la torsion se mueve sola.
        let azimut = azimut + 0.12 * (elevacion * 7.0 + self.tiempo * 0.5).sin() + 0.05 * (elevacion * 17.0 - self.tiempo * 0.9).sin();

        // Todo deriva lento, y un poco mas rapido cuando la cancion empuja.
        let deriva = self.tiempo * (0.045 + 0.02 * self.swell);

        // DOS CORTINAS, una delante de la otra, cada una con su borde de
        // abajo ondulado. El borde de abajo es lo mas brillante y lo mas
        // NITIDO de una aurora de verdad; hacia arriba la luz se deshilacha.
        let mut total = 0.0f32;
        let mut subida_pesada = 0.0f32;
        // Dos cortinas siempre, y una tercera ALTA, rojo magenta, que solo
        // aparece con las voces: las auroras intensas tienen ese borde rojo
        // arriba de todo.
        let capas = if self.swell > 0.05 { 3 } else { 2 };
        for capa in 0..capas {
            let c = capa as f32;
            let base = 0.07 + c * 0.16 + if capa == 2 { 0.12 } else { 0.0 }
                + AURORA_ONDA * ((azimut * (1.3 + c * 0.6) + deriva * (1.7 - c)).sin() * 0.6
                    + (azimut * 3.7 - deriva * 2.3 + c * 2.0).sin() * 0.3
                    + (azimut * 8.9 + deriva * 3.1).sin() * 0.1);
            // En el golpe la cortina se ESTIRA hacia arriba y vuelve.
            let alto = (0.40 + 0.16 * (azimut * 2.1 + deriva * 0.7 + c * 1.3).sin()) * (1.0 + 0.30 * self.pulso);
            let x = (elevacion - base) / alto;
            if !(-0.08..1.0).contains(&x) {
                continue;
            }
            // Filo abajo, cola larga arriba.
            let filo = ((x + 0.08) / 0.1).clamp(0.0, 1.0);
            let cola = (-x * 2.6).exp() * (1.0 - suave((x - 0.7) / 0.3));
            // Los rayos verticales: finos, brillantes y que se corren solos.
            let rayo = (0.5 + 0.5 * (azimut * (46.0 + c * 17.0) + 3.0 * (azimut * 6.0 + deriva).sin() + deriva * 5.0).sin())
                .powi(3);
            // Y TITILAN: cada rayo prende y apaga rapido, desfasado de los
            // vecinos, como las auroras que bailan.
            let titila = 0.75 + 0.25 * (self.tiempo * 5.0 + azimut * 31.0 + c * 2.0).sin();
            let estrias = (0.35 + 0.65 * rayo) * titila;
            // LOS PULSOS: una banda de brillo sube por la cortina en cada
            // tiempo del tema, del filo a la cima.
            let banda = (self.fase_beat + azimut * 0.15).fract();
            let pulso_sube = 1.0 + 0.9 * (-((x - banda * 1.1) / 0.09).powi(2)).exp() * (0.4 + 0.6 * self.pulso);
            // Los pliegues: la cortina se dobla y donde se dobla se ve mas.
            let pliegue = 0.45 + 0.55 * (0.5 + 0.5 * (azimut * 4.0 + deriva * 1.1 + (azimut * 9.0).sin() * 0.8).sin());
            let peso = filo * cola * estrias * pliegue * pulso_sube
                * if capa == 2 { 0.55 * self.swell } else { 1.0 - c * 0.45 };
            total += peso;
            // La capa alta va con la cima (el color rojo magenta).
            subida_pesada += if capa == 2 { peso } else { x.max(0.0) * peso };
        }
        // EL RESPLANDOR: un velo difuso que rodea la cortina y enciende el
        // cielo alrededor, mas ancho y mas tenue.
        let resplandor = 0.10 * (1.0 - (elevacion / 0.55).min(1.0)).powi(2);
        total += resplandor;
        if total <= 0.0 {
            return (0.0, 0.0);
        }

        // No da la vuelta entera: de un lado del cielo esta encendida y del
        // otro casi no, y el lado encendido gira despacio.
        let lobulo = 0.50 + 0.50 * (0.5 + 0.5 * (azimut - deriva * 0.8).sin());
        // Con las voces el lado apagado tambien se enciende: en el climax la
        // aurora llena el cielo entero, mire la camara para donde mire.
        let lobulo = lobulo + (1.0 - lobulo) * self.swell * 0.85;
        let respira = 0.85 + 0.15 * (self.tiempo * 0.57).sin();

        // Las olas del arpa: dos frentes por ataque que corren hacia los
        // lados desde su punto de salida, apagandose.
        let mut ola = 0.0f32;
        for &(salida, edad) in &self.olas {
            let vida = (1.0 - edad / OLA_VIDA).powf(1.5);
            for sentido in [-1.0f32, 1.0] {
                let frente = salida + sentido * OLA_VELOCIDAD * edad;
                let d = ((azimut - frente + PI).rem_euclid(2.0 * PI) - PI) / OLA_ANCHO;
                ola += (-d * d).exp() * vida;
            }
        }

        (total * lobulo * respira * (1.0 + 1.6 * ola) * self.fuerza_aurora(), subida_pesada / total)
    }

    /// LA CORONA BOREAL: rayos que convergen hacia el cenit, encima de todo.
    /// Solo existe en el climax. Devuelve el brillo en esta direccion.
    fn corona(&self, azimut: f32, elevacion: f32) -> f32 {
        if self.corona <= 0.0 || elevacion < 0.22 {
            return 0.0;
        }
        let deriva = self.tiempo * 0.25;
        let rayo = (0.5 + 0.5 * (azimut * 22.0 + deriva + (azimut * 3.0).sin() * 2.0).sin()).powi(5);
        let sube = ((elevacion - 0.22) / 0.5).clamp(0.0, 1.0);
        let cerca_del_cenit = 1.0 - ((elevacion - 1.2) / 0.35).clamp(0.0, 1.0) * 0.7;
        self.corona * rayo * sube * sube * cerca_del_cenit * (0.8 + 0.4 * self.pulso)
    }

    fn cerca_de_la_luna(&self, d: &Vec3) -> bool {
        // `d` ya viene con el giro deshecho, asi que se compara contra la
        // luna en su posicion de origen.
        d.dot(&normalize(&LUNA_DIRECCION)) > (LUNA_RADIO * 5.0).cos()
    }

    /// Hacia donde esta la luna AHORA, con el giro del cielo aplicado.
    /// Hacia donde esta el sol, en el mundo (gira con el cielo).
    pub fn sol(&self) -> Vec3 {
        girar_y(&normalize(&SOL_DIRECCION), self.giro)
    }

    /// Cuanto amanecio, de 0 a 1. Lo usa el destello de lente.
    pub fn amanecer(&self) -> f32 {
        self.amanecer
    }

    pub fn luna(&self) -> Vec3 {
        girar_y(&normalize(&LUNA_DIRECCION), self.giro)
    }

    /// LO QUE APORTA UNA ESTRELLA FUGAZ en la direccion `d`, de 0 a 1.
    ///
    /// La estrella recorre un ARCO DE CIRCULO MAXIMO, que es como cruza el
    /// cielo cualquier cosa que va derecho: se elige un punto de entrada
    /// `a` y una direccion de marcha `b` perpendicular a el, y la cabeza en
    /// cada momento es `a*cos(t) + b*sin(t)`. Los dos vectores salen del
    /// hash del numero del ataque, asi que cada nota del arpa entra por su
    /// propio lugar del cielo y siempre por el mismo.
    ///
    /// Para saber si `d` cae sobre la estela no hace falta recorrerla:
    /// alcanza con pasar `d` al sistema del arco. Su componente fuera del
    /// plano (contra `a x b`) dice a que distancia esta de la linea, y el
    /// angulo dentro del plano dice a que altura del recorrido. Son tres
    /// productos punto: la estela entera se resuelve sin iterar.
    fn estrella_fugaz(&self, d: &Vec3, numero: u32, edad: f32) -> f32 {
        // El punto de entrada y la direccion de marcha, del hash.
        let h1 = hash2(numero, 7717);
        let h2 = hash2(numero, 3391);
        let h3 = hash2(numero, 9173);

        // ENTRAN POR DONDE SE VE EL CIELO, que en esta escena es una
        // franja y no media esfera.
        //
        // La fuente esta adentro de una cueva con techo: casi todo el
        // cielo esta tapado, y lo que se ve es la banda que asoma por
        // encima del muro del fondo, hacia -Z, que es justo hacia donde
        // mira la camara. La luna esta puesta ahi (azimut 4.4, elevacion
        // 0.19) por la misma razon.
        //
        // Con el azimut al azar sobre toda la vuelta, cinco de cada seis
        // estrellas caian detras de una pared y no las veia nadie: el
        // arpa lanzaba y no pasaba nada. Sesgandolas a la ventana visible
        // se ven casi todas.
        // LA VENTANA SALE DE LA GEOMETRIA DE LA CAMARA, no del gusto.
        //
        // El ojo esta a unas once unidades del centro y mira al punto
        // (0, 2, 0) desde una altura de 4.7, o sea catorce grados y medio
        // HACIA ABAJO, con medio campo vertical de 22.5. El cuadro cubre
        // entonces de -37 a +8 grados de elevacion. Por abajo, el muro del
        // fondo (que llega a y = 3.2 a doce unidades) tapa todo lo que
        // este por debajo de unos -4. Asi que el cielo se ve en una franja
        // de unos doce grados alrededor del horizonte, y nada mas.
        //
        // La primera version puso las estrellas entre +10 y +42 grados,
        // que suena razonable para un cielo y esta ENTERO por encima del
        // borde de arriba del cuadro: se lanzaban, el test confirmaba que
        // encendian su arco, y no se veia ni una.
        //
        // Y ENCIMA HAY QUE DESCONTAR EL LETTERBOX. Sobre el final del
        // tema las bandas negras del formato ancho se comen un 11% de la
        // altura arriba y abajo, o sea otros cuatro grados y medio de
        // elevacion por arriba. El techo visible baja de +8 a +3.7.
        //
        // Con la franja util entre -3.8 (el borde del muro) y +3.7 (la
        // banda negra), lo que queda son siete grados pegados al
        // horizonte. La segunda version puso las estrellas entre +0.6 y
        // +7.5 grados y la mitad de arriba caia adentro de la banda
        // negra: se lanzaban, estaban en el cuadro, y el letterbox las
        // tapaba. Se encontro imprimiendo la elevacion de la cabeza y
        // comparandola contra lo que el encuadre deja ver.
        //
        // En horizontal el cuadro cubre el eje de la camara (azimut 4.71,
        // o sea -Z) mas menos 29 grados.
        const AZIMUT_CAMARA: f32 = 4.71;
        const ABANICO: f32 = 0.50;
        let azimut = AZIMUT_CAMARA + (h1 - 0.5) * 2.0 * ABANICO;
        let elevacion = -0.02 + h2 * 0.075;
        let a = direccion(azimut, elevacion);

        // Una direccion de marcha perpendicular a `a`. Se arma con el
        // producto cruz contra un eje auxiliar y se gira un angulo al azar
        // dentro del plano, asi que no todas caen igual.
        let aux = if a.y.abs() < 0.9 {
            Vec3::new(0.0, 1.0, 0.0)
        } else {
            Vec3::new(1.0, 0.0, 0.0)
        };
        // `u` es la tangente HORIZONTAL (sale del producto cruz contra el
        // eje vertical) y `v` la que sube. La marcha se arma sobre todo
        // con la horizontal y un poco de la vertical: una estrella que
        // sube o baja a plomo se sale de la franja visible en medio
        // segundo, y una que cruza en diagonal la recorre entera. Que sea
        // en diagonal es ademas como se ven: casi nunca caen a plomo.
        let u = normalize(&cross(&a, &aux));
        let v = cross(&a, &u);
        let lado = if h3 < 0.5 { 1.0 } else { -1.0 };
        let caida = -(0.15 + h3.fract() * 0.45);
        let b = normalize(&(u * lado + v * caida));

        // Donde esta la cabeza AHORA, en radianes de arco.
        let fraccion = edad / ESTRELLA_VIDA;
        let cabeza = fraccion * ESTRELLA_CARRERA;

        // `d` en el sistema del arco.
        let n = cross(&a, &b);
        let fuera = dot(d, &n).abs();
        if fuera > ESTRELLA_GROSOR * 3.0 {
            return 0.0;
        }
        let ang = dot(d, &b).atan2(dot(d, &a));

        // Detras de la cabeza y no mas de `ESTRELLA_ESTELA`: ahi esta la
        // estela. `atras` va de 0 en la cabeza a 1 en la cola.
        let atras = (cabeza - ang) / ESTRELLA_ESTELA;
        if !(0.0..1.0).contains(&atras) {
            return 0.0;
        }

        // El perfil a lo ancho: gaussiano, y mas FINO hacia la cola. Una
        // estela real se abre un poco al alejarse de la cabeza, pero se
        // apaga mucho mas rapido de lo que se abre, asi que en cuadro lo
        // que se ve es una punta que se afila.
        let ancho = ESTRELLA_GROSOR * (1.0 - atras * 0.55);
        let perfil = (-(fuera / ancho) * (fuera / ancho)).exp();

        // A lo largo: brillante en la cabeza y apagandose hacia la cola.
        let cola = (1.0 - atras).powi(3);

        // Y la vida entera entra y sale con una curva en S, para que no
        // aparezca ni desaparezca de golpe.
        let vida = {
            let x = (1.0 - (fraccion * 2.0 - 1.0).abs()).clamp(0.0, 1.0);
            x * x * (3.0 - 2.0 * x)
        };

        perfil * cola * vida
    }

    /// El color del cielo en la direccion `d` (normalizada).
    pub fn color(&self, d: &Vec3) -> Color {
        // El giro del cielo se aplica al reves sobre la direccion: girar
        // el cielo `giro` a la derecha es lo mismo que mirar `giro` a la
        // izquierda en la imagen fija.
        let d = girar_y(d, -self.giro);

        // Longitud y latitud de la direccion, a UV equirectangulares.
        let u = 0.5 + d.z.atan2(d.x) / (2.0 * PI);
        let v = 0.5 - d.y.clamp(-1.0, 1.0).asin() / PI;
        let u = u.rem_euclid(1.0);
        let v = v.clamp(0.0, 1.0);
        let mut noche = self.imagen.muestrear(u, v);

        // EL TITILEO. El cielo esta horneado, asi que una estrella no puede
        // parpadear por su cuenta: lo que se hace es modular el brillo de
        // los texeles que YA son mucho mas claros que el fondo, que son las
        // estrellas y nada mas (la nebulosa no llega ni a la mitad del
        // umbral). La fase sale de un hash de la CELDA de ocho por ocho
        // texeles en la que cae el punto, no del texel: una estrella ocupa
        // varios texeles por el filtrado bilineal, y con una fase por texel
        // sus pedazos parpadearian desacompasados y se veria como ruido.
        //
        // La luna queda afuera por el corte de arriba: es lo unico tan
        // brillante como una estrella y ocupa demasiado como para que
        // parpadear le quede bien.
        let brillo = noche.r as u32 + noche.g as u32 + noche.b as u32;
        if brillo > TITILEO_UMBRAL && !self.cerca_de_la_luna(&d) {
            let celda_x = (u * ANCHO as f32 / 8.0) as u32;
            let celda_y = (v * ALTO as f32 / 8.0) as u32;
            let fase = hash2(celda_x + 601, celda_y + 197) * 6.283;
            // Entre el 65% y el 100%: parpadean, no se apagan.
            let f = 0.825 + (self.tiempo * TITILEO_VELOCIDAD + fase).sin() * 0.175;
            let canal = |c: u8| (c as f32 * f) as u8;
            noche = Color::new(canal(noche.r), canal(noche.g), canal(noche.b), 255);
        }

        // LA AURORA, cuando las voces se sostienen. Va encima del cielo
        // horneado y debajo de las estrellas fugaces, que es lo que es: un
        // velo de luz, no un objeto. Se apaga con el dia por lo mismo que
        // las fugaces, aunque en este tema el swell cae entero de noche.
        //
        // El angulo no se vuelve a calcular: `u` y `v` YA son la longitud y
        // la latitud de la direccion, asi que salen de dos multiplicaciones
        // y no de un `atan2` y un `asin` mas por rayo.
        if self.fuerza_aurora() > 0.0 {
            let (fuerza, subida) = self.aurora((u - 0.5) * 2.0 * PI, (0.5 - v) * PI);
            if fuerza > 0.0 {
                // Verde intenso en el filo, turquesa en el cuerpo y magenta
                // arriba: la paleta de una aurora de verdad, que ademas es la
                // de la fuente.
                let [filo, cuerpo, cima] = self.colores;
                let (r, g, b) = if subida < 0.35 {
                    mezcla(filo, cuerpo, subida / 0.35)
                } else {
                    mezcla(cuerpo, cima, ((subida - 0.35) / 0.5).min(1.0))
                };
                let f = fuerza.min(1.6);
                let suma = |base: u8, c: f32| (base as f32 + c * f).min(255.0) as u8;
                noche = Color::new(suma(noche.r, r), suma(noche.g, g), suma(noche.b, b), 255);
            }
        }

        // LA CORONA del climax, encima de la aurora.
        let corona = self.corona((u - 0.5) * 2.0 * PI, (0.5 - v) * PI);
        if corona > 0.0 {
            let suma = |base: u8, c: f32| (base as f32 + c * corona).min(255.0) as u8;
            noche = Color::new(suma(noche.r, 255.0), suma(noche.g, 170.0), suma(noche.b, 245.0), 255);
        }

        // EL COMETA, encima de la aurora: gira con las estrellas (esta en el
        // cielo girado) y se apaga con el dia.
        let cometa = self.cometa(&d);
        let de_noche = (1.0 - self.amanecer).max(0.0);
        if cometa.3 > 0.0 && de_noche > 0.0 {
            let suma = |base: u8, c: f32| (base as f32 + c * de_noche).min(255.0) as u8;
            noche = Color::new(suma(noche.r, cometa.0), suma(noche.g, cometa.1), suma(noche.b, cometa.2), 255);
        }

        // LAS ESTRELLAS FUGACES, sumadas encima de todo.
        //
        // Se apagan con la luz del dia: una estrella fugaz a plena luz no
        // se ve, y ademas seria una raya blanca sobre un cielo rosa, que
        // se leeria como un defecto y no como una estrella. Con el cielo
        // encendido del alba desaparecen solas.
        let mut fugaz = 0.0f32;
        if self.amanecer < 0.95 && !self.estrellas.is_empty() {
            for &(numero, edad) in &self.estrellas {
                fugaz += self.estrella_fugaz(&d, numero, edad);
            }
            fugaz *= 1.0 - self.amanecer;
        }
        let con_fugaz = |c: Color| {
            if fugaz <= 0.0 {
                return c;
            }
            // Blanco apenas azulado y SUMADO: es luz.
            let f = fugaz.min(1.5);
            let mezcla = |base: u8, k: f32| (base as f32 + 255.0 * f * k).min(255.0) as u8;
            Color::new(mezcla(c.r, 0.92), mezcla(c.g, 0.96), mezcla(c.b, 1.0), 255)
        };

        // EL MAR DE NUBES, debajo del horizonte. Va encima de todo lo del
        // cielo (tambien del alba, que si no lavaria las nubes a lila) y
        // debajo de las fugaces.
        let nublar = |c: Color| {
            if d.y < NUBES_TECHO.sin() {
                self.con_nubes(c, u, d.y.clamp(-1.0, 1.0).asin())
            } else {
                c
            }
        };

        if self.amanecer <= 0.0 {
            return con_fugaz(nublar(noche));
        }

        // El amanecer, en DOS CAPAS, y hacen falta las dos.
        //
        // La de abajo es la franja del horizonte: un resplandor que sube
        // desde la linea del suelo y se apaga con la altura, oro pegado al
        // horizonte y rosa mas arriba.
        //
        // La de arriba es la BOVEDA ENTERA. Mientras el amanecer era un
        // detalle del ultimo cuarto del tema alcanzaba con la franja, pero
        // ahora la escena ABRE de dia y tiene que leerse como de dia, y un
        // cielo que esta rosa abajo y negro con estrellas arriba no es un
        // amanecer: es una noche con una luz rara en el borde. A las cinco
        // de la manana el cenit no es negro, es un azul profundo pero
        // CLARO, y las estrellas de arriba se apagan igual que las de
        // abajo, solo que mas tarde.
        //
        // Asi que primero toda la boveda se lleva hacia ese azul, y encima
        // va la franja. El orden importa: al reves, la boveda lavaria el
        // oro del horizonte.
        let elevacion = d.y.clamp(-1.0, 1.0).asin();

        // --- Capa 1: la boveda ---
        //
        // El azul del cenit al alba. Mezclado por `amanecer * 0.72`: aun a
        // pleno dia queda casi un tercio de la noche debajo, y eso es lo
        // que deja ver todavia alguna estrella grande y la nebulosa
        // fantasma. Un cielo de amanecer completamente liso se ve pintado.
        const CENIT: (f32, f32, f32) = (54.0, 62.0, 112.0);
        // Y mas claro cuanto mas bajo se mire, que es como es: el aire
        // dispersa mas cerca del horizonte.
        let bajo = (1.0 - (elevacion / 0.9).clamp(0.0, 1.0)).powf(1.6);
        let dome = self.amanecer * 0.72;
        let boveda = |base: u8, cenit: f32| -> f32 {
            let destino = cenit + (170.0 - cenit) * bajo * 0.55;
            base as f32 + (destino - base as f32) * dome
        };
        let (mut r, mut g, mut b) = (
            boveda(noche.r, CENIT.0),
            boveda(noche.g, CENIT.1),
            boveda(noche.b, CENIT.2),
        );

        // --- Capa 2: la franja del horizonte ---
        let altura = (1.0 - elevacion / 0.45).clamp(0.0, 1.0);
        let franja = altura * altura * self.amanecer;
        let oro = (1.0 - elevacion / 0.12).clamp(0.0, 1.0) * franja;

        let canal = |base: f32, rosa: f32, dorado: f32| {
            (base * (1.0 - franja * 0.55) + rosa * franja + dorado * oro).clamp(0.0, 255.0) as u8
        };
        r = canal(r, 205.0, 110.0) as f32;
        g = canal(g, 110.0, 75.0) as f32;
        b = canal(b, 150.0, 20.0) as f32;

        // --- Capa 3: EL SOL ---
        // Un disco casi blanco y calido, un resplandor dorado alrededor y un
        // halo ancho que tine el cielo de oro, como el sol bajo de los
        // atardeceres del juego. Va encima de las nubes: esta sobre ellas.
        let al_sol = d.dot(&normalize(&SOL_DIRECCION)).clamp(-1.0, 1.0).acos();
        let disco = (1.0 - ((al_sol - 0.028) / 0.006).clamp(0.0, 1.0)) * 255.0;
        let brillo = (-(al_sol / 0.09).powi(2)).exp() * 170.0 + (-al_sol / 0.45).exp() * 60.0;
        let k = self.amanecer;
        let sol = |c: Color| {
            let suma = |base: u8, v: f32| (base as f32 + v * k).min(255.0) as u8;
            Color::new(
                suma(c.r, disco + brillo),
                suma(c.g, disco * 0.95 + brillo * 0.78),
                suma(c.b, disco * 0.80 + brillo * 0.42),
                255,
            )
        };

        con_fugaz(sol(nublar(Color::new(r as u8, g as u8, b as u8, 255))))
    }
}

impl Cielo {
    /// LOS COMETAS: tres, repartidos alrededor del cielo para que mientras
    /// la camara gira alrededor de la isla casi siempre haya uno a la vista
    /// (la camara mira un poco hacia abajo y del cielo entra solo una
    /// franja baja). Cada uno con una cabeza blanca y DOS colas, como los de
    /// verdad: la de iones, recta y fina, y la de polvo, mas ancha y curvada,
    /// cada cola de su color: el clasico (celeste y oro), el de las hadas
    /// (rosa y violeta) y el de la aurora (turquesa y verde). Estan fijos en
    /// el cielo de las estrellas (giran con la noche) y avanzan apenas por su
    /// cuenta. Devuelve el color a sumar (r, g, b) y cuanto hay, para poder
    /// saltearlo. Son un punado de productos punto por rayo de cielo.
    fn cometa(&self, d: &Vec3) -> (f32, f32, f32, f32) {
        // (azimut, elevacion, largo de la cola, color de iones, color de polvo)
        type Cometa = (f32, f32, f32, (f32, f32, f32), (f32, f32, f32));
        const COMETAS: [Cometa; 3] = [
            (-2.35, 0.13, 0.45, (0.55, 0.80, 1.00), (1.00, 0.72, 0.62)),
            (0.20, 0.11, 0.38, (0.95, 0.55, 1.00), (1.00, 0.55, 0.75)),
            (2.10, 0.15, 0.32, (0.45, 1.00, 0.85), (0.70, 1.00, 0.55)),
        ];
        let avanza = self.tiempo * 0.0012;
        let (mut r, mut g, mut b) = (0.0f32, 0.0f32, 0.0f32);
        for &(az, elev, largo, iones_c, polvo_c) in &COMETAS {
            let cabeza = direccion(az + avanza, elev + avanza * 0.2);
            let cerca = d.dot(&cabeza);
            if cerca < 0.80 {
                continue;
            }
            // En el plano tangente a la cabeza: `v` es donde esta `d` visto
            // desde la cabeza. La cola sale en diagonal, mas de costado que
            // hacia arriba, para que entre en el cuadro.
            let v = *d - cabeza * cerca;
            let arriba = Vec3::new(0.0, 1.0, 0.0);
            let lado = normalize(&cross(&cabeza, &arriba));
            let eje = normalize(&(lado * 0.85 + (arriba - cabeza * cabeza.y) * 0.45));
            let a_lo_largo = v.dot(&eje);
            let de_costado = (v - eje * a_lo_largo).magnitude();
            let angulo = v.magnitude();

            // La cabeza y la coma.
            let nucleo = (-(angulo / 0.010).powi(2)).exp() * 280.0 + (-(angulo / 0.05).powi(2)).exp() * 90.0;
            r += nucleo;
            g += nucleo;
            b += nucleo * 1.05;

            if a_lo_largo > 0.0 {
                let desvanece = (-a_lo_largo / largo).exp();
                let ancho = 0.010 + 0.07 * a_lo_largo;
                let iones = (-(de_costado / ancho).powi(2)).exp() * desvanece * 170.0;
                // La de polvo se curva hacia el costado a medida que se aleja.
                let curva = (v - (eje * a_lo_largo + lado * (0.35 * a_lo_largo * a_lo_largo))).magnitude();
                let ancho_polvo = 0.018 + 0.16 * a_lo_largo;
                let polvo = (-(curva / ancho_polvo).powi(2)).exp() * desvanece * 140.0;
                r += iones * iones_c.0 + polvo * polvo_c.0;
                g += iones * iones_c.1 + polvo * polvo_c.1;
                b += iones * iones_c.2 + polvo * polvo_c.2;
            }
        }
        // Titilan apenas, como si respiraran.
        let late = 0.9 + 0.1 * (self.tiempo * 1.7).sin();
        (r * late, g * late, b * late, r + g + b)
    }

    /// El cielo en `base`, con el mar de nubes encima si en esa direccion
    /// hay. `u` es la longitud (la misma del cielo) y `elevacion` la de la
    /// direccion, que aca siempre es baja.
    fn con_nubes(&self, base: Color, u: f32, elevacion: f32) -> Color {
        let v = ((NUBES_TECHO - elevacion) / (NUBES_TECHO - NUBES_PISO)).clamp(0.0, 1.0);
        let x = ((u * NUBES_ANCHO as f32) as usize).min(NUBES_ANCHO - 1);
        let y = ((v * (NUBES_ALTO - 1) as f32) as usize).min(NUBES_ALTO - 1);
        let (densidad, luz) = self.nubes[y * NUBES_ANCHO + x];
        if densidad <= 0.0 {
            return base;
        }

        // El color de las nubes segun la hora. Tres tonos por hora: el de la
        // copa que le da la luz, el de la panza en sombra, y el del VACIO
        // que se ve entre nube y nube, que es mas profundo que las dos. Sin
        // los tres la capa era un velo rosa parejo: el contraste entre la
        // copa encendida y el hueco oscuro es lo que la hace nube.
        let dia = self.amanecer;
        let aurora = self.fuerza_aurora();
        let copa = mezcla((90.0, 100.0, 175.0), (255.0, 222.0, 188.0), dia);
        let panza = mezcla((22.0, 24.0, 58.0), (150.0, 112.0, 168.0), dia);
        let (mut r, mut g, mut b) = mezcla(panza, copa, luz);
        r += 15.0 * aurora * luz;
        g += 110.0 * aurora * luz;
        b += 75.0 * aurora * luz;

        let vacio = mezcla(
            (base.r as f32 * 0.8, base.g as f32 * 0.8, base.b as f32 * 0.9),
            (92.0, 70.0, 138.0),
            dia * 0.8,
        );

        // Pegado al horizonte, las nubes se funden con el resplandor del
        // cielo en vez de cortar.
        let horizonte = (1.0 - (-elevacion / 0.05)).clamp(0.0, 1.0);
        let k = densidad;
        let mezclado = |v: f32, n: f32| v + (n - v) * k;
        let (fr, fg, fb) = (mezclado(vacio.0, r), mezclado(vacio.1, g), mezclado(vacio.2, b));
        let canal = |c: u8, n: f32| (n + (c as f32 - n) * horizonte * 0.6).clamp(0.0, 255.0) as u8;
        Color::new(canal(base.r, fr), canal(base.g, fg), canal(base.b, fb), 255)
    }
}

/// Hornea el mar de nubes.
///
/// Cada texel es una direccion que mira hacia abajo; esa direccion corta un
/// plano de nubes que esta muy por debajo de la isla, y el ruido se evalua
/// EN ESE PUNTO DEL PLANO, no en el texel. Eso es lo que da la perspectiva:
/// las nubes de abajo se ven grandes y las del horizonte se aplastan y se
/// achican. Cerca del horizonte el punto se va al infinito y el ruido se
/// volveria granito, asi que ahi se lo funde con su promedio.
///
/// La luz es la diferencia de densidad con un punto corrido hacia el sol: lo
/// que tiene mas nube del lado del sol esta en sombra, y lo que tiene menos,
/// en la copa iluminada. Es la receta vieja de las nubes en 2D.
fn hornear_nubes() -> Vec<(f32, f32)> {
    const ALTURA: f32 = 16.0;
    const ESCALA: f32 = 11.0;
    let fbm = |x: f32, z: f32| {
        let mut suma = 0.0;
        let mut peso = 0.5;
        let mut f = 1.0;
        for o in 0..5 {
            suma += ruido_valor(x * f, z * f, 71 + o) * peso;
            f *= 2.03;
            peso *= 0.5;
        }
        suma / 0.97
    };
    let mut nubes = Vec::with_capacity(NUBES_ANCHO * NUBES_ALTO);
    for y in 0..NUBES_ALTO {
        let v = (y as f32 + 0.5) / NUBES_ALTO as f32;
        let elevacion = NUBES_TECHO - v * (NUBES_TECHO - NUBES_PISO);
        for x in 0..NUBES_ANCHO {
            let u = (x as f32 + 0.5) / NUBES_ANCHO as f32;
            let azimut = (u - 0.5) * 2.0 * PI;
            if elevacion >= -0.002 {
                // Sobre el horizonte, solo un velo que se apaga.
                let velo = (1.0 - elevacion / NUBES_TECHO).clamp(0.0, 1.0) * 0.55;
                nubes.push((velo, 0.4));
                continue;
            }
            let distancia = (ALTURA / (-elevacion).tan()).min(600.0);
            let (px, pz) = (azimut.cos() * distancia / ESCALA, azimut.sin() * distancia / ESCALA);
            let lejos = (distancia / 220.0).clamp(0.0, 1.0);
            let n = fbm(px, pz) * (1.0 - lejos) + 0.52 * lejos;
            let densidad = suave((n - 0.44) / 0.18);
            let corrido = fbm(px + 0.35, pz - 0.2) * (1.0 - lejos) + 0.52 * lejos;
            let luz = (0.5 + (n - corrido) * 7.0).clamp(0.0, 1.0) * (0.35 + 0.65 * densidad);
            // Al horizonte, un colchon parejo.
            let densidad = densidad * (1.0 - lejos) + 0.85 * lejos;
            nubes.push((densidad, luz));
        }
    }
    nubes
}

/// Ruido de valor continuo en el plano, en [0, 1].
fn ruido_valor(x: f32, z: f32, semilla: u32) -> f32 {
    let (x0, z0) = (x.floor(), z.floor());
    let (tx, tz) = (x - x0, z - z0);
    let (sx, sz) = (tx * tx * (3.0 - 2.0 * tx), tz * tz * (3.0 - 2.0 * tz));
    let h = |a: f32, b: f32| hash2((a as i32) as u32 ^ semilla.wrapping_mul(0x9e37_79b9), (b as i32) as u32);
    let arriba = h(x0, z0) + (h(x0 + 1.0, z0) - h(x0, z0)) * sx;
    let abajo = h(x0, z0 + 1.0) + (h(x0 + 1.0, z0 + 1.0) - h(x0, z0 + 1.0)) * sx;
    arriba + (abajo - arriba) * sz
}

/// Gira un vector alrededor del eje Y.
pub fn girar_y(v: &Vec3, angulo: f32) -> Vec3 {
    let (s, c) = angulo.sin_cos();
    Vec3::new(v.x * c + v.z * s, v.y, -v.x * s + v.z * c)
}

/// Direccion unitaria a partir de azimut (sobre XZ) y elevacion. Inversa del
/// mapeo de `Cielo::color`: azimut = atan2(z, x).
fn direccion(azimut: f32, elevacion: f32) -> Vec3 {
    let c = elevacion.cos();
    Vec3::new(c * azimut.cos(), elevacion.sin(), c * azimut.sin())
}

fn mezcla(a: (f32, f32, f32), b: (f32, f32, f32), t: f32) -> (f32, f32, f32) {
    (
        a.0 + (b.0 - a.0) * t,
        a.1 + (b.1 - a.1) * t,
        a.2 + (b.2 - a.2) * t,
    )
}

fn suave(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Numero repetible en [0, 1] a partir de dos enteros.
fn hash2(x: u32, y: u32) -> f32 {
    let mut h = x.wrapping_mul(374_761_393) ^ y.wrapping_mul(668_265_263);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    ((h ^ (h >> 16)) & 0xFFFF) as f32 / 65535.0
}

/// Ruido fractal de `ancho` x `alto` en [0, 1], que cierra en U (el cielo
/// da la vuelta) con `celdas` celdas a lo ancho en la primera octava.
fn campo(ancho: usize, alto: usize, celdas: usize, octavas: usize, semilla: u32) -> Vec<f32> {
    let mut campo = vec![0.0f32; ancho * alto];
    let mut peso = 1.0f32;
    let mut suma = 0.0f32;
    let mut cx = celdas.max(1);

    for octava in 0..octavas {
        let cy = (cx / 2).max(1);
        let sem = semilla.wrapping_mul(7919).wrapping_add(octava as u32 * 104_729);
        for y in 0..alto {
            for x in 0..ancho {
                let fx = x as f32 * cx as f32 / ancho as f32;
                let fy = y as f32 * cy as f32 / alto as f32;
                let (x0, y0) = (fx.floor() as usize, fy.floor() as usize);
                let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
                let sx = tx * tx * (3.0 - 2.0 * tx);
                let sy = ty * ty * (3.0 - 2.0 * ty);

                let v = |gx: usize, gy: usize| hash2((gx % cx) as u32 + sem, gy as u32 + (sem >> 8));
                let arriba = v(x0, y0) + (v(x0 + 1, y0) - v(x0, y0)) * sx;
                let abajo = v(x0, y0 + 1) + (v(x0 + 1, y0 + 1) - v(x0, y0 + 1)) * sx;
                campo[y * ancho + x] += (arriba + (abajo - arriba) * sy) * peso;
            }
        }
        suma += peso;
        peso *= 0.5;
        cx *= 2;
    }

    for c in campo.iter_mut() {
        *c /= suma;
    }
    campo
}

#[cfg(test)]
mod tests {
    use super::*;

    /// La luna esta donde dice `LUNA_DIRECCION`: mirando ahi el cielo es
    /// casi blanco, y mirando al lado opuesto es oscuro.
    #[test]
    fn la_luna_esta_donde_se_la_puso() {
        let cielo = Cielo::generar();
        let hacia = cielo.color(&normalize(&LUNA_DIRECCION));
        let contra = cielo.color(&normalize(&-LUNA_DIRECCION));
        assert!(hacia.r > 200 && hacia.g > 200);
        assert!(contra.r < 120 && contra.b < 160);
    }

    /// El horizonte es mas claro que el cenit.
    /// Con amanecer el horizonte se calienta (mas rojo que azul) y con el
    /// cielo girado la luna se corre.
    #[test]
    fn el_amanecer_calienta_y_el_giro_mueve_la_luna() {
        let mut cielo = Cielo::generar();
        let horizonte = direccion(1.0, 0.03);
        let noche = cielo.color(&horizonte);
        cielo.ajustar(0.0, 1.0, 0.0, &[], 0.0);
        let alba = cielo.color(&horizonte);
        assert!(alba.r > noche.r + 60 && alba.r > alba.b);

        cielo.ajustar(0.6, 0.0, 0.0, &[], 0.0);
        let luna = cielo.luna();
        assert!((luna - normalize(&LUNA_DIRECCION)).magnitude() > 0.3);
        // Mirando a donde esta la luna ahora, se la ve.
        let c = cielo.color(&luna);
        assert!(c.r > 200);
    }

    /// Las estrellas parpadean y la luna no.
    #[test]
    fn las_estrellas_titilan_y_la_luna_no() {
        let mut cielo = Cielo::generar();

        // Se busca una estrella barriendo el cielo.
        let mut estrella = None;
        for i in 0..4000 {
            // Solo sobre el horizonte: abajo estan las nubes, que no titilan.
            let d = direccion(i as f32 * 0.37, 0.1 + (i as f32 * 0.11).sin().abs() * 0.8);
            let c = cielo.color(&d);
            if c.r as u32 + c.g as u32 + c.b as u32 > TITILEO_UMBRAL + 60 && !cielo.cerca_de_la_luna(&d) {
                estrella = Some(d);
                break;
            }
        }
        let estrella = estrella.expect("el cielo tiene que tener estrellas");

        // A lo largo de un ciclo, su brillo cambia de verdad.
        let brillo = |c: &Cielo, d: &Vec3| {
            let x = c.color(d);
            x.r as i32 + x.g as i32 + x.b as i32
        };
        let mut min = i32::MAX;
        let mut max = i32::MIN;
        for paso in 0..24 {
            cielo.ajustar(0.0, 0.0, paso as f32 * 0.3, &[], 0.0);
            let b = brillo(&cielo, &estrella);
            min = min.min(b);
            max = max.max(b);
        }
        assert!(max - min > 30, "la estrella no titila: {min}..{max}");

        // La luna se queda quieta.
        let luna = normalize(&LUNA_DIRECCION);
        cielo.ajustar(0.0, 0.0, 0.0, &[], 0.0);
        let a = brillo(&cielo, &luna);
        cielo.ajustar(0.0, 0.0, 1.8, &[], 0.0);
        assert_eq!(a, brillo(&cielo, &luna));
    }

    /// La estrella fugaz tiene que ENCENDER el cielo a lo largo de su
    /// arco y no tocarlo fuera de el. Se verifica sin depender de la
    /// escena: se arma una estrella a mano, se busca el punto mas
    /// brillante del cielo y se comprueba que supera con holgura al cielo
    /// sin ella.
    #[test]
    fn la_estrella_fugaz_enciende_su_arco() {
        let mut cielo = Cielo::generar();

        // Se compara DIRECCION POR DIRECCION, no el maximo del cielo. El
        // maximo no sirve: la luna ya vale blanco puro, asi que con
        // estrella o sin ella el maximo es el mismo 765 y el test no
        // mediria nada. (Se escribio asi primero y paso exactamente eso.)
        let malla: Vec<Vec3> = (0..240)
            .flat_map(|a| {
                (0..60).map(move |e| {
                    direccion(a as f32 / 240.0 * std::f32::consts::TAU, e as f32 / 60.0 * 1.5)
                })
            })
            .collect();
        let brillo = |c: Color| c.r as i32 + c.g as i32 + c.b as i32;

        cielo.ajustar(0.0, 0.0, 0.0, &[], 0.0);
        let sin: Vec<i32> = malla.iter().map(|d| brillo(cielo.color(d))).collect();

        // Con una estrella a la mitad de su vida, que es cuando mas
        // brilla. Las salidas llegan ya elegidas desde el analisis.
        let t = ESTRELLA_VIDA * 0.5;
        cielo.ajustar(0.0, 0.0, t, &[(0, 0.0)], 0.0);
        assert_eq!(cielo.estrellas.len(), 1, "no se armo la estrella");
        let con: Vec<i32> = malla.iter().map(|d| brillo(cielo.color(d))).collect();

        let subida = con.iter().zip(&sin).map(|(c, s)| c - s).max().unwrap();
        assert!(
            subida > 200,
            "la estrella no enciende nada: la mayor subida fue de {subida} sobre 765"
        );

        // Y tiene que ser una RAYA: pocas direcciones tocadas. Si tocara
        // muchas seria un velo sobre el cielo, no una estrella.
        let tocadas = con.iter().zip(&sin).filter(|(c, s)| *c - *s > 30).count();
        assert!(
            (1..malla.len() / 30).contains(&tocadas),
            "toca {tocadas} de {} direcciones: tiene que ser una raya fina",
            malla.len()
        );
    }

    /// Fuera de su vida no existe, y de dia tampoco.
    #[test]
    fn la_estrella_fugaz_no_existe_fuera_de_su_vida() {
        let mut cielo = Cielo::generar();
        cielo.ajustar(0.0, 0.0, ESTRELLA_VIDA + 0.5, &[(0, 0.0)], 0.0);
        assert!(cielo.estrellas.is_empty(), "sigue viva pasada su vida");

        cielo.ajustar(0.0, 0.0, -1.0, &[(0, 0.0)], 0.0);
        assert!(cielo.estrellas.is_empty(), "existe antes de lanzarse");
    }

    /// LA AURORA: vive toda la noche, tenue, y las voces la encienden; de
    /// dia no existe y nunca llega al cenit. Y no es un filtro de color: la
    /// cortina tiene lados.
    #[test]
    fn la_aurora_vive_de_noche_y_crece_con_las_voces() {
        let mut cielo = Cielo::generar();
        let banda: Vec<Vec3> = (0..360)
            .map(|a| direccion(a as f32 / 360.0 * std::f32::consts::TAU, 0.22))
            .collect();
        let cenit: Vec<Vec3> = (0..36)
            .map(|a| direccion(a as f32 / 36.0 * std::f32::consts::TAU, 1.4))
            .collect();
        let brillo = |c: &Cielo, ds: &[Vec3]| -> i32 {
            ds.iter().map(|d| { let x = c.color(d); x.r as i32 + x.g as i32 + x.b as i32 }).sum()
        };

        // Sin aurora de referencia: de dia.
        cielo.ajustar(0.0, 1.0, 130.0, &[], 1.0);
        let dia_con = brillo(&cielo, &banda);
        let mut sin = Cielo::generar();
        sin.ajustar(0.0, 1.0, 130.0, &[], 0.0);
        assert_eq!(dia_con, brillo(&sin, &banda), "de dia no hay aurora");

        cielo.ajustar(0.0, 0.0, 130.0, &[], 0.0);
        let noche = brillo(&cielo, &banda);
        let cenit_noche = brillo(&cielo, &cenit);
        let mut apagado = Cielo::generar();
        apagado.ajustar(0.0, 0.0, 130.0, &[], 0.0);
        apagado.amanecer = 1.0;
        cielo.ajustar(0.0, 0.0, 130.0, &[], 1.0);
        let voces = brillo(&cielo, &banda);
        assert!(voces > noche + 3000, "las voces no la encienden: {noche} -> {voces}");
        assert_eq!(brillo(&cielo, &cenit), cenit_noche, "la aurora llego al cenit");

        let por_azimut: Vec<i32> = banda
            .iter()
            .map(|d| { let c = cielo.color(d); c.r as i32 + c.g as i32 + c.b as i32 })
            .collect();
        let (mas, menos) = (*por_azimut.iter().max().unwrap(), *por_azimut.iter().min().unwrap());
        assert!(mas > menos * 2, "la aurora esta pareja en todo el cielo: {menos} a {mas}");
    }

    #[test]
    fn el_cometa_brilla_en_su_cabeza_y_no_lejos() {
        let cielo = Cielo::generar();
        let cabeza = direccion(-2.35, 0.13);
        let (_, _, _, ahi) = cielo.cometa(&cabeza);
        let (_, _, _, lejos) = cielo.cometa(&direccion(-1.0, 0.9));
        eprintln!("cometa: cabeza {ahi}, lejos {lejos}");
        assert!(ahi > 200.0 && lejos == 0.0);
        let c = cielo.color(&cabeza);
        eprintln!("color en la cabeza: {:?}", (c.r, c.g, c.b));
    }

    #[test]
    fn el_horizonte_es_mas_claro() {
        let cielo = Cielo::generar();
        let mut horizonte = 0u32;
        let mut cenit = 0u32;
        for i in 0..32 {
            let a = i as f32 / 32.0 * 2.0 * PI;
            let h = cielo.color(&direccion(a, 0.02));
            horizonte += h.r as u32 + h.g as u32 + h.b as u32;
        }
        let c = cielo.color(&Vec3::new(0.0, 1.0, 0.0));
        cenit += c.r as u32 + c.g as u32 + c.b as u32;
        assert!(horizonte / 32 > cenit);
    }
}
