use crate::ray_intersect::{Intersect, RayIntersect};
use nalgebra_glm::{dot, Vec3};

/// Un puñado de objetos metidos adentro de una esfera envolvente.
///
/// La escena no tiene estructura de aceleracion: cada rayo prueba contra
/// TODOS los objetos, uno por uno. Eso se paga carisimo con piezas hechas
/// de muchos triangulos chiquitos y juntos (la espada son 41), porque un
/// rayo que pasa a metros de distancia igual las prueba una por una.
///
/// El grupo corta eso con una sola cuenta: si el rayo no toca la esfera que
/// las contiene, no puede tocar ninguna, y las 41 pruebas se saltean de un
/// saque. Si la toca, recien ahi se prueba adentro y no se pierde nada.
///
/// OJO: la esfera tiene que contener a TODOS los hijos. Si queda corta, las
/// partes que sobresalen desaparecen de la escena sin avisar.
pub struct Group {
    center: Vec3,
    radius: f32,
    children: Vec<Box<dyn RayIntersect + Send + Sync>>,
}

impl Group {
    pub fn new(center: Vec3, radius: f32, children: Vec<Box<dyn RayIntersect + Send + Sync>>) -> Self {
        Group {
            center,
            radius,
            children,
        }
    }

    /// Solo dice SI el rayo cruza la esfera, no donde. Sirve para descartar,
    /// asi que no hace falta calcular el punto de impacto.
    ///
    /// Contempla que el origen este ADENTRO: en ese caso la raiz mas lejana
    /// es positiva aunque la mas cercana quede atras, y el rayo si cruza.
    fn hits_bounds(&self, origin: &Vec3, direction: &Vec3) -> bool {
        let oc = origin - self.center;

        // Con la direccion normalizada, el coeficiente cuadratico es 1.
        let b = 2.0 * dot(&oc, direction);
        let c = dot(&oc, &oc) - self.radius * self.radius;

        let discriminant = b * b - 4.0 * c;
        if discriminant < 0.0 {
            return false;
        }

        // Basta con que la interseccion mas lejana quede adelante del rayo.
        let sqrt_d = discriminant.sqrt();
        (-b + sqrt_d) / 2.0 > 1e-3
    }
}

impl RayIntersect for Group {
    fn ray_intersect(&self, origin: &Vec3, direction: &Vec3) -> Intersect {
        if !self.hits_bounds(origin, direction) {
            return Intersect::empty();
        }

        // Adentro del grupo se hace lo mismo que en el bucle principal de la
        // escena: gana el impacto mas cercano.
        let mut nearest = f32::INFINITY;
        let mut result = Intersect::empty();

        for child in &self.children {
            let hit = child.ray_intersect(origin, direction);
            if hit.is_intersecting && hit.distance < nearest {
                nearest = hit.distance;
                result = hit;
            }
        }

        result
    }
}
