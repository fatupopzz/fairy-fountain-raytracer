use crate::texture::{Texture, TextureImage};
use raylib::prelude::Color;
use std::sync::Arc;

/// Como responde una superficie a la luz.
/// La geometria dice DONDE pego el rayo; el material dice de que color sale.
// Sin Copy: la textura de imagen lleva un Arc adentro, que no es Copy.
// El clon igual es barato, solo sube el contador del Arc.
#[derive(Debug, Clone)]
pub struct Material {
    /// [peso difuso, peso especular, peso de reflexion, peso de refraccion].
    /// Cuanto aporta cada termino al color final.
    pub albedo: [f32; 4],
    /// Exponente del brillo. Mas alto = brillo mas pequenio y concentrado.
    pub specular: f32,
    /// Indice de refraccion del medio (aire 1.0, agua 1.33, vidrio ~1.5).
    pub refractive_index: f32,
    /// Patron procedural que da el color base segun las UV del impacto.
    pub texture: Texture,
    /// Luz propia del material. Se suma tal cual al final del sombreado,
    /// sin importar las luces ni las sombras: el objeto brilla solo.
    pub emission_color: Option<Color>,
    /// Relieve: un mapa de normales y cuantas veces se repite sobre el
    /// rango UV de la superficie. `None` deja la normal geometrica.
    ///
    /// Es lo que hace que la piedra tenga grano y el marmol vetas que la
    /// luz recorre, sin agregar un solo triangulo: el sombreado inclina la
    /// normal segun el mapa y el difuso y el especular responden a eso.
    pub relieve: Option<(Arc<TextureImage>, f32)>,
    /// CUANTO BORROSO ES EL REFLEJO, de 0 (espejo perfecto) a ~0.3.
    ///
    /// Un espejo perfecto es lo que sale gratis en un trazador, y es
    /// justamente lo que delata a un render: en el mundo no hay espejos
    /// perfectos salvo los espejos. Una losa de marmol pulido refleja, pero
    /// su reflejo esta ESTIRADO y desenfocado porque la superficie, vista
    /// de cerca, no es un plano sino un campo de microfacetas apuntando a
    /// direcciones parecidas pero no iguales. Con reflexion perfecta el
    /// piso de la plaza se leia como vidrio y el agua como plastico.
    ///
    /// El rayo reflejado se desvia un punto al azar de una esfera de este
    /// radio (ver `cast_ray`), y el acumulador temporal promedia las
    /// desviaciones de los ultimos cuadros. O sea que el desenfoque sale
    /// por CERO rayos extra: es el mismo rayo de siempre, apuntado un poco
    /// distinto en cada cuadro.
    pub rugosidad: f32,
}

impl Material {
    pub fn new(
        albedo: [f32; 4],
        specular: f32,
        refractive_index: f32,
        texture: Texture,
        emission_color: Option<Color>,
    ) -> Self {
        Material {
            albedo,
            specular,
            refractive_index,
            texture,
            emission_color,
            relieve: None,
            rugosidad: 0.0,
        }
    }

    /// El mismo material con relieve: `mapa` es el mapa de normales y
    /// `repeticiones` cuantas veces entra en el rango UV de la superficie.
    /// El mismo material con el reflejo desenfocado. Ver `rugosidad`.
    pub fn con_rugosidad(mut self, rugosidad: f32) -> Self {
        self.rugosidad = rugosidad;
        self
    }

    pub fn con_relieve(mut self, mapa: Arc<TextureImage>, repeticiones: f32) -> Self {
        self.relieve = Some((mapa, repeticiones));
        self
    }
}
