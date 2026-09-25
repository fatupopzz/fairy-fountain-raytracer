//! La linea de tiempo de la cancion.
//!
//! Aca no se toca ni un objeto ni una luz: esto solo responde "en el segundo
//! t, ¿como tiene que estar la escena?". Quien aplica esos numeros a la
//! escena de verdad es `animacion.rs`, y quien decide que segundo es se
//! define en `audio.rs`.
//!
//! Esa separacion es a proposito. Los keyframes se ajustan a oido mirando
//! solo esta tabla, sin tener que entender como esta armada la escena.

/// "Loser" esta a ~110 pulsos por minuto.
pub const BPM: f32 = 110.0;

/// Cuanto dura un pulso, en segundos. 60 / 110 = 0.545.
pub const BEAT: f32 = 60.0 / BPM;

/// Largo para el que estan escritos los tiempos de la tabla: 3:50.
///
/// Es una REFERENCIA, no el largo real del archivo. Si el mp3 dura otra
/// cosa, `escalar` estira o encoge la tabla entera para que las secciones
/// caigan en la misma proporcion. Sin eso, con un tema de 268 s la tabla se
/// reiniciaria a los 230 y los ultimos 38 segundos irian montados sobre la
/// intro.
pub const DURACION_REFERENCIA: f32 = 230.0;

/// Cuanto dura el golpe del pulso, en segundos. Corto: es un golpe, no una
/// respiracion. Pasado de ~0.15 los golpes se pisan entre si y en vez de
/// latido queda un temblor.
pub const PULSO_ATAQUE: f32 = 0.1;

/// Cuanto sube la emision de los anillos en el pico del golpe. 0.3 = +30%.
pub const PULSO_FUERZA: f32 = 0.3;

/// Cuantas luces puntuales tiene la escena. La tabla de abajo escribe una
/// intensidad por cada una, en el mismo orden en que se construyen.
pub const LUCES: usize = 6;

/// El estado completo de la escena en un instante.
///
/// Todo lo de aca son MULTIPLICADORES sobre el valor con el que se
/// construyo la escena, no valores absolutos. Asi la tabla se lee sola
/// ("las luces al 70%") y cambiar el color o la potencia de una luz en la
/// escena no obliga a rehacer los doce keyframes.
#[derive(Clone, Debug)]
pub struct SceneParams {
    /// Multiplica la intensidad del bloom.
    pub bloom_strength: f32,
    /// Densidad de la niebla, este si es absoluto (va directo al exponente).
    pub fog_density: f32,
    /// Un multiplicador por luz puntual. Siempre `LUCES` de largo.
    pub light_intensities: Vec<f32>,
    /// Multiplica la emision de las esferas de los dos anillos.
    pub ring_emission: f32,
    /// Multiplica la emision de los haces de laser.
    pub laser_emission: f32,
    /// Radianes por segundo de la orbita automatica de la camara.
    pub camera_drift_speed: f32,
}

impl SceneParams {
    /// Mezcla lineal entre dos estados. `t` va de 0 (todo `a`) a 1 (todo `b`).
    fn mezclar(a: &SceneParams, b: &SceneParams, t: f32) -> SceneParams {
        let lerp = |x: f32, y: f32| x + (y - x) * t;

        SceneParams {
            bloom_strength: lerp(a.bloom_strength, b.bloom_strength),
            fog_density: lerp(a.fog_density, b.fog_density),
            light_intensities: a
                .light_intensities
                .iter()
                .zip(b.light_intensities.iter())
                .map(|(&x, &y)| lerp(x, y))
                .collect(),
            ring_emission: lerp(a.ring_emission, b.ring_emission),
            laser_emission: lerp(a.laser_emission, b.laser_emission),
            camera_drift_speed: lerp(a.camera_drift_speed, b.camera_drift_speed),
        }
    }
}

/// Atajo para no escribir doce veces el nombre de cada campo.
///
/// El orden de las luces es el de la escena:
///   0 rosa intensa, 1 cian, 2 magenta, 3 blanca calida, 4 rosa suave,
///   5 azul profundo de abajo.
fn kf(
    bloom: f32,
    fog: f32,
    luces: [f32; LUCES],
    anillos: f32,
    laseres: f32,
    deriva: f32,
) -> SceneParams {
    SceneParams {
        bloom_strength: bloom,
        fog_density: fog,
        light_intensities: luces.to_vec(),
        ring_emission: anillos,
        laser_emission: laseres,
        camera_drift_speed: deriva,
    }
}

/// La cancion entera, en orden de tiempo.
///
/// Entre dos filas se interpola lineal, asi que la separacion entre filas ES
/// la duracion de la transicion: 58 -> 65 son siete segundos de build, y
/// 195 -> 220 son veinticinco de bajada.
pub fn tabla() -> Vec<(f32, SceneParams)> {
    vec![
        // INTRO: casi todo apagado. La escena arranca como un escenario a
        // oscuras antes de que entre la banda, y la camara ni se mueve.
        (0.0, kf(0.3, 0.15, [0.3; LUCES], 0.4, 0.2, 0.0)),
        // Empieza a levantar.
        (15.0, kf(0.5, 0.17, [0.5; LUCES], 0.6, 0.35, 0.02)),
        // VERSO 1: energia media y la camara arranca a orbitar.
        (30.0, kf(0.7, 0.20, [0.7; LUCES], 0.8, 0.6, 0.05)),
        // PRE-CORO: el build.
        (58.0, kf(0.9, 0.24, [0.9; LUCES], 1.1, 0.8, 0.07)),
        // CORO 1: explosion. Las luces pasan de 1.0 a proposito, para que
        // los pixeles se vayan arriba de 255 y el bloom reviente.
        (65.0, kf(1.5, 0.30, [1.2; LUCES], 1.5, 1.0, 0.10)),
        // VERSO 2: baja, pero queda por encima del verso 1.
        (95.0, kf(0.8, 0.22, [0.75; LUCES], 0.9, 0.65, 0.06)),
        // PRE-CORO 2.
        (120.0, kf(1.1, 0.26, [1.0; LUCES], 1.2, 0.85, 0.08)),
        // CORO 2: mas fuerte que el primero.
        (128.0, kf(1.8, 0.32, [1.35; LUCES], 1.7, 1.1, 0.12)),
        // PUENTE: el momento intimo. Solo quedan la rosa (0), la magenta
        // (2) y la rosa suave (4); la cian y la blanca se apagan al 10% y
        // ahi la escena cambia de color entera. La camara casi se detiene.
        (160.0, kf(0.4, 0.10, [0.6, 0.1, 0.55, 0.1, 0.5, 0.25], 0.3, 0.15, 0.015)),
        // BUILD FINAL.
        (185.0, kf(1.0, 0.22, [1.0; LUCES], 1.1, 0.7, 0.06)),
        // CORO FINAL: el maximo de todo el tema.
        (195.0, kf(2.0, 0.35, [1.5; LUCES], 2.0, 1.2, 0.14)),
        // OUTRO: veinticinco segundos bajando hasta el estado de la intro.
        (220.0, kf(0.6, 0.20, [0.5; LUCES], 0.6, 0.3, 0.03)),
        // Cierre EXACTO en el estado del keyframe 0. Sin esta fila el
        // salto del loop se veria como un corte: el ultimo cuadro de la
        // cancion y el primero tienen que ser el mismo.
        (DURACION_REFERENCIA, kf(0.3, 0.15, [0.3; LUCES], 0.4, 0.2, 0.0)),
    ]
}

/// Estira (o encoge) la tabla para que entre exactamente en `largo` segundos.
///
/// Los tiempos se escalan todos por el mismo factor, asi que la ESTRUCTURA se
/// conserva: el primer coro sigue cayendo al 28% del tema y el puente al 70%.
/// No es lo mismo que medir los cortes a oido sobre el archivo de verdad
/// (para eso hay que corregir la tabla a mano), pero deja las secciones
/// repartidas donde corresponde en vez de fuera de lugar.
pub fn escalar(tabla: &mut [(f32, SceneParams)], largo: f32) {
    if largo <= 0.0 {
        return;
    }

    let factor = largo / DURACION_REFERENCIA;
    for (t, _) in tabla.iter_mut() {
        *t *= factor;
    }
}

/// El estado de la escena en el segundo `t`.
///
/// `t` se envuelve solo con `duracion`, asi que da igual si el reloj viene
/// pasado: la cancion se repite.
pub fn evaluar(tabla: &[(f32, SceneParams)], t: f32, duracion: f32) -> SceneParams {
    debug_assert!(!tabla.is_empty(), "la tabla de keyframes no puede estar vacia");

    let t = t.rem_euclid(duracion.max(1e-3));

    // Antes del primer keyframe (no deberia pasar, el primero esta en 0.0).
    if t <= tabla[0].0 {
        return tabla[0].1.clone();
    }

    for par in tabla.windows(2) {
        let (t0, ref a) = par[0];
        let (t1, ref b) = par[1];

        if t < t1 {
            // Dos keyframes en el mismo segundo serian un corte seco; el
            // maximo evita ademas dividir entre cero.
            let span = (t1 - t0).max(1e-6);
            return SceneParams::mezclar(a, b, (t - t0) / span);
        }
    }

    // Despues del ultimo.
    tabla[tabla.len() - 1].1.clone()
}

/// El latido del ritmo, aparte de los keyframes.
///
/// Devuelve 1.0 casi todo el tiempo y salta a 1.3 justo en el pulso, cayendo
/// en linea recta durante `PULSO_ATAQUE`. Es una funcion PURA del tiempo: no
/// guarda estado, no cuenta pulsos y no se desincroniza por mas que la
/// cancion corra media hora.
///
///   pulso = 1 + fuerza * max(0, 1 - (t % beat) / ataque)
pub fn pulso(t: f32) -> f32 {
    let desde_el_golpe = t.rem_euclid(BEAT);
    let caida = (1.0 - desde_el_golpe / PULSO_ATAQUE).max(0.0);

    1.0 + PULSO_FUERZA * caida
}
