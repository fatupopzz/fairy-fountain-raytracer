//! Un cubo que se puede GIRAR.
//!
//! `Cube` es una caja alineada a los ejes: seis planos que no se mueven, y
//! por eso el test es tan barato. Para posar a un personaje eso no alcanza:
//! un brazo que sostiene la ocarina esta inclinado, el sombrero de Link cae
//! hacia atras en diagonal y el escudo cuelga torcido sobre la espalda.
//!
//! El truco es no girar la caja sino el RAYO. La caja vive en su propio
//! sistema de coordenadas, centrada en el origen y alineada a sus ejes, y
//! lo que se hace es llevar el rayo a ese sistema: se le resta el centro y
//! se lo proyecta sobre los tres ejes de la caja. Ahi adentro el problema es
//! el de siempre (el test de las tres losas de `Cube`), y la normal que sale
//! se vuelve a llevar al mundo con los mismos ejes. Como los ejes son
//! ortonormales, llevar y traer es proyectar: tres productos punto.
//!
//! Tiene una cara especial, la +Z local, que puede llevar OTRO material.
//! Es lo que permite pintar una cara en la cabeza de Link o el emblema en
//! el frente del escudo sin que las otras cinco caras lo repitan.

use crate::material::Material;
use crate::ray_intersect::{Intersect, RayIntersect};
use crate::vec3::{cross, dot, normalize, Vec3};

const EPSILON: f32 = 0.001;

pub struct CajaOrientada {
    pub centro: Vec3,
    /// Los tres ejes de la caja en el mundo, ortonormales: x, y, z locales.
    pub ejes: [Vec3; 3],
    /// La mitad del tamano en cada eje local.
    pub medio: Vec3,
    pub material: Material,
    /// El material de la cara que mira hacia +Z local, si es distinto.
    pub frente: Option<Material>,
    /// Si es mayor que cero, las UV se repiten cada `1 / mosaico` unidades
    /// en vez de estirarse una vez por cara. Igual que en `Cube`. Link no lo
    /// usa (sus texturas son de una pieza por cara), pero es del engine.
    pub mosaico: f32,
}

/// Una rotacion como sus tres ejes: primero `guinada` alrededor de Y,
/// despues `cabeceo` alrededor de la X ya girada y por ultimo `alabeo`
/// alrededor de la Z. Es el orden de un avion y el que resulta natural
/// para posar un cuerpo: hacia donde mira, cuanto se inclina hacia
/// adelante y cuanto se ladea.
pub fn ejes_de(guinada: f32, cabeceo: f32, alabeo: f32) -> [Vec3; 3] {
    let x = Vec3::new(1.0, 0.0, 0.0);
    let y = Vec3::new(0.0, 1.0, 0.0);
    let z = Vec3::new(0.0, 0.0, 1.0);
    let [x, y, z] = [x, y, z].map(|v| girar(&v, &Vec3::new(0.0, 0.0, 1.0), alabeo));
    let [x, y, z] = [x, y, z].map(|v| girar(&v, &Vec3::new(1.0, 0.0, 0.0), cabeceo));
    [x, y, z].map(|v| girar(&v, &Vec3::new(0.0, 1.0, 0.0), guinada))
}

/// Gira `v` un angulo alrededor del eje unitario `eje` (Rodrigues).
pub fn girar(v: &Vec3, eje: &Vec3, angulo: f32) -> Vec3 {
    let (s, c) = angulo.sin_cos();
    v * c + cross(eje, v) * s + eje * (dot(eje, v) * (1.0 - c))
}

/// Compone dos rotaciones dadas como ejes: `hijo` esta expresado en el
/// sistema de `padre`, y se devuelve en el del mundo.
pub fn componer(padre: &[Vec3; 3], hijo: &[Vec3; 3]) -> [Vec3; 3] {
    hijo.map(|e| llevar(padre, &e))
}

/// Lleva un vector del sistema de `ejes` al del mundo.
pub fn llevar(ejes: &[Vec3; 3], v: &Vec3) -> Vec3 {
    ejes[0] * v.x + ejes[1] * v.y + ejes[2] * v.z
}

impl CajaOrientada {
    pub fn nueva(centro: Vec3, tam: Vec3, ejes: [Vec3; 3], material: Material) -> Self {
        CajaOrientada {
            centro,
            ejes,
            medio: tam * 0.5,
            material,
            frente: None,
            mosaico: 0.0,
        }
    }

    pub fn con_frente(mut self, material: Material) -> Self {
        self.frente = Some(material);
        self
    }

    /// Mueve la caja entera: nuevo centro y nuevos ejes. Es lo que llama la
    /// animacion en cada cuadro.
    pub fn colocar(&mut self, centro: Vec3, ejes: [Vec3; 3]) {
        self.centro = centro;
        self.ejes = ejes;
    }

    /// El rayo en el sistema de la caja.
    fn local(&self, origen: &Vec3, dir: &Vec3) -> (Vec3, Vec3) {
        let o = origen - self.centro;
        (
            Vec3::new(dot(&o, &self.ejes[0]), dot(&o, &self.ejes[1]), dot(&o, &self.ejes[2])),
            Vec3::new(dot(dir, &self.ejes[0]), dot(dir, &self.ejes[1]), dot(dir, &self.ejes[2])),
        )
    }

    /// Las tres losas, igual que `Cube::slabs`. Devuelve la distancia y el
    /// eje de la cara por la que entra o, si el origen esta adentro, por la
    /// que sale.
    fn losas(&self, o: &Vec3, d: &Vec3) -> Option<(f32, usize)> {
        let mut entra = f32::NEG_INFINITY;
        let mut sale = f32::INFINITY;
        let mut eje_entra = 0;
        let mut eje_sale = 0;

        for eje in 0..3 {
            let inv = 1.0 / d[eje];
            let mut a = (-self.medio[eje] - o[eje]) * inv;
            let mut b = (self.medio[eje] - o[eje]) * inv;
            if a > b {
                std::mem::swap(&mut a, &mut b);
            }
            if a > entra {
                entra = a;
                eje_entra = eje;
            }
            if b < sale {
                sale = b;
                eje_sale = eje;
            }
        }

        if entra >= sale || sale <= EPSILON {
            return None;
        }
        if entra > EPSILON {
            Some((entra, eje_entra))
        } else {
            // Desde adentro sale por la cara de salida; la normal igual se
            // devuelve contra el rayo, como hace `Cube`, que es lo que
            // espera la refraccion.
            Some((sale, eje_sale))
        }
    }

    fn caja_mundo(&self) -> (Vec3, Vec3) {
        // El alcance en cada eje del mundo es la suma de lo que aporta cada
        // eje de la caja proyectado sobre el.
        let mut r = Vec3::zeros();
        for k in 0..3 {
            r[k] = (0..3).map(|e| (self.ejes[e][k] * self.medio[e]).abs()).sum();
        }
        (self.centro - r, self.centro + r)
    }
}

impl RayIntersect for CajaOrientada {
    fn ray_intersect<'a>(&'a self, origin: &Vec3, direction: &Vec3) -> Intersect<'a> {
        let (o, d) = self.local(origin, direction);
        let Some((t, eje)) = self.losas(&o, &d) else {
            return Intersect::empty();
        };

        let punto_local = o + d * t;
        // La normal va siempre contra el rayo, como en `Cube`.
        let normal = normalize(&(self.ejes[eje] * (-d[eje].signum())));

        // UV de la cara: los otros dos ejes locales.
        let (a, b) = match eje {
            0 => (2, 1),
            1 => (0, 2),
            _ => (0, 1),
        };
        let (u, v) = if self.mosaico > 0.0 {
            (
                (punto_local[a] + self.medio[a]) * self.mosaico,
                (punto_local[b] + self.medio[b]) * self.mosaico,
            )
        } else {
            (
                ((punto_local[a] + self.medio[a]) / (2.0 * self.medio[a])).clamp(0.0, 1.0),
                // V crece hacia ABAJO en las imagenes: se da vuelta para que
                // la cara pintada no quede cabeza abajo.
                (1.0 - (punto_local[b] + self.medio[b]) / (2.0 * self.medio[b])).clamp(0.0, 1.0),
            )
        };

        // La cara +Z local, vista desde afuera.
        let material = match &self.frente {
            Some(m) if eje == 2 && punto_local.z > 0.0 => m,
            _ => &self.material,
        };

        let punto = origin + direction * t;
        Intersect::new(punto, normal, t, material, u, v)
    }

    fn puede_tapar(&self) -> bool {
        self.material.emission_color.is_none()
    }

    fn occluded(&self, origin: &Vec3, direction: &Vec3, max_distance: f32) -> bool {
        if self.material.emission_color.is_some() {
            return false;
        }
        let (o, d) = self.local(origin, direction);
        matches!(self.losas(&o, &d), Some((t, _)) if t < max_distance)
    }

    fn transmision(&self, origin: &Vec3, direction: &Vec3, max_distance: f32) -> f32 {
        if !self.occluded(origin, direction, max_distance) {
            return 1.0;
        }
        self.material.albedo[3]
    }

    fn aabb(&self) -> Option<(Vec3, Vec3)> {
        Some(self.caja_mundo())
    }

    fn bounds(&self) -> Option<(Vec3, f32)> {
        Some((self.centro, self.medio.norm()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::texture::Texture;
    use raylib::prelude::Color;

    fn material() -> Material {
        Material::new([1.0, 0.0, 0.0, 0.0], 1.0, 0.0, Texture::Solid(Color::WHITE), None)
    }

    /// Sin girar, es exactamente el `Cube` de siempre.
    #[test]
    fn sin_giro_es_un_cubo() {
        let c = CajaOrientada::nueva(Vec3::zeros(), Vec3::new(2.0, 2.0, 2.0), ejes_de(0.0, 0.0, 0.0), material());
        let hit = c.ray_intersect(&Vec3::new(0.0, 0.0, -5.0), &Vec3::new(0.0, 0.0, 1.0));
        assert!(hit.is_intersecting);
        assert!((hit.distance - 4.0).abs() < 1e-4);
        assert!((hit.normal - Vec3::new(0.0, 0.0, -1.0)).norm() < 1e-4);
    }

    /// Girada 45 grados, un rayo que antes pegaba en la cara pega en la
    /// ARISTA, mas cerca: a raiz de dos en vez de a uno del centro.
    #[test]
    fn girada_pega_en_la_arista() {
        let c = CajaOrientada::nueva(
            Vec3::zeros(),
            Vec3::new(2.0, 2.0, 2.0),
            ejes_de(std::f32::consts::FRAC_PI_4, 0.0, 0.0),
            material(),
        );
        let hit = c.ray_intersect(&Vec3::new(0.0, 0.0, -5.0), &Vec3::new(0.0, 0.0, 1.0));
        assert!(hit.is_intersecting);
        assert!((hit.distance - (5.0 - 2.0f32.sqrt())).abs() < 1e-3, "{}", hit.distance);
        // Y la caja del mundo crece para abarcarla.
        let (min, max) = c.aabb().unwrap();
        assert!((max.x - 2.0f32.sqrt()).abs() < 1e-4 && (min.z + 2.0f32.sqrt()).abs() < 1e-4);
    }

    /// La cara +Z local usa el material del frente, las demas no.
    #[test]
    fn el_frente_tiene_su_material() {
        let mut cara = material();
        cara.specular = 99.0;
        let c = CajaOrientada::nueva(Vec3::zeros(), Vec3::new(2.0, 2.0, 2.0), ejes_de(0.0, 0.0, 0.0), material())
            .con_frente(cara);
        let de_frente = c.ray_intersect(&Vec3::new(0.0, 0.0, 5.0), &Vec3::new(0.0, 0.0, -1.0));
        let de_atras = c.ray_intersect(&Vec3::new(0.0, 0.0, -5.0), &Vec3::new(0.0, 0.0, 1.0));
        assert_eq!(de_frente.material.specular, 99.0);
        assert_eq!(de_atras.material.specular, 1.0);
    }
}
