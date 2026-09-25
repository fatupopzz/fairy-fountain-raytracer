//! La musica y, sobre todo, EL RELOJ.
//!
//! Todo lo que se mueve en la escena es una funcion de un solo numero: en
//! que segundo de la cancion estamos. Este modulo es el unico que sabe de
//! donde sale ese numero, y tiene dos fuentes:
//!
//!   - con el mp3 cargado, el tiempo REAL de reproduccion que reporta
//!     raylib. Es lo unico que mantiene los keyframes pegados a la musica:
//!     si un cuadro tarda 300 ms, el tiempo salta 300 ms y la escena salta
//!     con el, en vez de irse quedando atras;
//!   - sin el mp3, un reloj interno desde el arranque. La escena se ve
//!     igual, solo que no hay nada que escuchar.
//!
//! LA API DE RAYLIB-RS NO ES LA DE C. Los nombres de aca salen de leer
//! raylib-5.5.1/src/core/audio.rs, no de traducir la documentacion de C:
//! `play_stream`, `update_stream`, `pause_stream`, `resume_stream`,
//! `is_stream_playing`, `get_time_played`, `get_time_length`, `seek_stream`.

use raylib::prelude::{Music, RaylibAudio};
use std::path::Path;
use std::time::Instant;

/// Volumen del tema. Bajo: la idea es que acompanie, no que tape.
pub const VOLUMEN: f32 = 0.55;

/// Cuantos frames de audio deja raylib cargados por delante, por sub-buffer.
///
/// ESTO ES LO QUE EVITA QUE LA MUSICA SE CORTE, y hay que entender por que
/// para no romperlo de nuevo.
///
/// raylib reproduce desde dos sub-buffers que se van rellenando cuando se
/// llama a `update_stream`. Por defecto cada uno tiene sampleRate/30 frames,
/// o sea 1470: entre los dos, 67 ms de musica adelantada. Si el programa
/// tarda mas que eso en volver a llamar a `update_stream`, el reproductor se
/// queda sin nada que sonar y ahi esta el corte.
///
/// Y aca un cuadro tarda ~350 ms: trazar 800 x 600 con cinco rebotes no es
/// gratis. Con el default, el sonido se cortaba tres veces por cuadro.
///
/// Con 16384 frames por sub-buffer son 372 ms cada uno y 744 ms entre los
/// dos: mas del doble del cuadro mas lento. Cuesta 262 KB de memoria y le
/// agrega esa demora al arranque del audio, que aca no le molesta a nadie.
///
/// OJO: tiene que llamarse ANTES de cargar la musica. Despues no hace nada,
/// porque el stream se dimensiona al abrirse.
const FRAMES_ADELANTADOS: i32 = 16384;

/// El reloj de la escena, con o sin musica.
///
/// El parametro de vida `'aud` no es decoracion: un `Music` de raylib-rs se
/// presta del `RaylibAudio` que abrio el dispositivo, y el compilador no
/// deja que el dispositivo se cierre antes que el stream. En la practica eso
/// obliga a declarar el `RaylibAudio` ANTES que este reloj en `main`, para
/// que Rust los suelte en el orden correcto.
pub struct RelojEscena<'aud> {
    musica: Option<Music<'aud>>,
    /// Desde cuando corre el tramo actual (solo importa sin musica).
    arranque: Instant,
    /// Segundos ya juntados en los tramos anteriores a la ultima pausa.
    acumulado: f32,
    corriendo: bool,
}

impl<'aud> RelojEscena<'aud> {
    /// Prueba las rutas en orden y se queda con la primera que exista. Si no
    /// hay dispositivo de audio o no aparece el archivo en ninguna, devuelve
    /// un reloj interno y el programa sigue igual: quedarse sin musica no es
    /// motivo para no abrir el raytracer.
    pub fn nuevo(audio: Option<&'aud RaylibAudio>, rutas: &[&str]) -> Self {
        // Se pregunta por el archivo ANTES de pedirselo a raylib: si no esta,
        // raylib escupe un error rojo en la consola que parece un problema de
        // verdad, y aca no lo es.
        let encontrada = rutas.iter().find(|r| Path::new(r).exists()).copied();

        let musica = match (audio, encontrada) {
            (Some(audio), Some(ruta)) => {
                // Antes de abrir el stream, no despues.
                audio.set_audio_stream_buffer_size_default(FRAMES_ADELANTADOS);

                match audio.new_music(ruta) {
                Ok(musica) => {
                    musica.set_volume(VOLUMEN);
                    musica.play_stream();
                    println!(
                        "musica: {ruta} ({:.1} s)",
                        musica.get_time_length()
                    );
                    Some(musica)
                }
                Err(e) => {
                    eprintln!("sin musica, no se pudo cargar {ruta}: {e}");
                    None
                }
                }
            }
            (Some(_), None) => {
                println!(
                    "sin musica: no aparecio el archivo en {rutas:?} \
                     (la escena corre igual, con reloj interno)"
                );
                None
            }
            (None, _) => None,
        };

        RelojEscena {
            musica,
            arranque: Instant::now(),
            acumulado: 0.0,
            corriendo: true,
        }
    }

    /// Va en CADA vuelta del loop. Si se saltea, el sonido se corta apenas
    /// se vacia lo que raylib ya tenia cargado por delante.
    pub fn actualizar(&self) {
        if let Some(musica) = &self.musica {
            musica.update_stream();
        }
    }

    /// El segundo de la cancion en el que estamos.
    ///
    /// Con musica sale del stream, asi que la escena y el sonido no se
    /// pueden separar por mucho que se atrase un cuadro. Sin musica es el
    /// reloj interno, que ademas respeta la pausa.
    pub fn tiempo(&self) -> f32 {
        match &self.musica {
            Some(musica) => musica.get_time_played(),
            None => {
                if self.corriendo {
                    self.acumulado + self.arranque.elapsed().as_secs_f32()
                } else {
                    self.acumulado
                }
            }
        }
    }

    /// Espacio: para y arranca.
    pub fn alternar_pausa(&mut self) {
        match &self.musica {
            Some(musica) => {
                if musica.is_stream_playing() {
                    musica.pause_stream();
                    self.corriendo = false;
                } else {
                    musica.resume_stream();
                    self.corriendo = true;
                }
            }
            None => {
                if self.corriendo {
                    // Se guarda lo corrido hasta aca y se frena.
                    self.acumulado += self.arranque.elapsed().as_secs_f32();
                    self.corriendo = false;
                } else {
                    // Se reanuda: el tramo nuevo arranca ahora.
                    self.arranque = Instant::now();
                    self.corriendo = true;
                }
            }
        }
    }

    pub fn corriendo(&self) -> bool {
        self.corriendo
    }

    pub fn hay_musica(&self) -> bool {
        self.musica.is_some()
    }

    /// Largo del tema cargado, si hay. Sirve para avisar cuando el mp3 no
    /// dura lo que la tabla de keyframes supone.
    pub fn duracion(&self) -> Option<f32> {
        self.musica.as_ref().map(|m| m.get_time_length())
    }
}
