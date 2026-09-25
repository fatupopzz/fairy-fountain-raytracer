use nalgebra_glm::Vec3;
use raylib::prelude::Color;

/// A cuantos `alcance` de distancia una luz deja de aportar del todo.
/// Tres es el equilibrio: mas chico y se ve el borde donde la luz se
/// termina, mas grande y deja de servir para descartar.
const CORTE: f32 = 3.0;

/// Luz puntual: un punto en el espacio que emite en todas direcciones.
pub struct Light {
    pub position: Vec3,
    pub color: Color,
    pub intensity: f32,
    /// Distancia a la que la luz cae a la mitad. La atenuacion es
    /// `1 / (1 + (d / alcance)^2)`: suave cerca, y sin el corte a cero de
    /// la ley del cuadrado inverso pura, que con pocas luces deja negro
    /// todo lo que no esta pegado a una.
    ///
    /// Sin atenuacion, una luz alumbraba igual el pedestal que la pared del
    /// fondo, y la cueva se leia como un diagrama; con ella la luz VIVE en
    /// un lugar y se apaga hacia afuera.
    pub alcance: f32,
}

impl Light {
    pub fn new(position: Vec3, color: Color, intensity: f32) -> Self {
        Light {
            position,
            color,
            intensity,
            alcance: 9.0,
        }
    }

    /// La misma luz con otro alcance: para las lejanas (la luna) que tienen
    /// que llegar a la escena entera pareja.
    pub fn con_alcance(mut self, alcance: f32) -> Self {
        self.alcance = alcance;
        self
    }

    /// Cuanto llega de esta luz a un punto a `distancia`.
    ///
    /// Es la caida cuadratica de siempre, PERO multiplicada por una
    /// ventana que llega a cero exacto en `alcance * CORTE`. Sin la
    /// ventana, `1 / (1 + d^2)` nunca se hace cero: una antorcha de
    /// alcance 3 todavia aporta un 3% a veinte unidades de distancia, y
    /// aunque eso no se vea, el trazador igual le tira su rayo de sombra
    /// contra todos los objetos. Medido: las dos antorchas costaban nueve
    /// milisegundos por cuadro alumbrando una escena entera en la que solo
    /// se las ve en su rincon.
    ///
    /// Con la ventana, `cast_ray` puede descartar la luz entera de una
    /// comparacion. Va a la cuarta potencia y al cuadrado para que la
    /// caida se acelere cerca del corte en vez de cortar de golpe: si el
    /// borde se notara, seria peor el remedio.
    pub fn atenuacion(&self, distancia: f32) -> f32 {
        let d = distancia / self.alcance;
        let x = (d / CORTE).min(1.0);
        let ventana = 1.0 - x * x * x * x;
        ventana * ventana / (1.0 + d * d)
    }
}
