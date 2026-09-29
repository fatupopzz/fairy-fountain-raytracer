use crate::material::Material;
use crate::ray_intersect::{Intersect, RayIntersect};
use crate::vec3::{dot, normalize, Vec3};
use std::f32::consts::PI;

pub struct Sphere {
    pub center: Vec3,
    pub radius: f32,
    pub material: Material,
}

impl RayIntersect for Sphere {
    fn ray_intersect<'a>(&'a self, origin: &Vec3, direction: &Vec3) -> Intersect<'a> {
        // Sustituye P(t) = O + tD en ||P - C||^2 = r^2 y despeja la cuadratica.
        let oc = origin - self.center;

        let a = dot(direction, direction); // = 1 si D esta normalizada
        let b = 2.0 * dot(direction, &oc);
        let c = dot(&oc, &oc) - self.radius * self.radius;

        let discriminant = b * b - 4.0 * a * c;

        if discriminant < 0.0 {
            return Intersect::empty(); // el rayo pasa de largo
        }

        let sqrt_d = discriminant.sqrt();
        let t1 = (-b - sqrt_d) / (2.0 * a); // cara de enfrente
        let t2 = (-b + sqrt_d) / (2.0 * a); // cara de atras

        // Solo cuenta lo que esta FRENTE a la camara.
        // El epsilon evita que un rayo se re-intersecte con la
        // superficie de la que acaba de salir.
        let distance = if t1 > 0.001 {
            t1
        } else if t2 > 0.001 {
            t2
        } else {
            return Intersect::empty();
        };

        // Aqui esta la diferencia entre un circulo y una esfera:
        let point = origin + direction * distance;
        let normal = normalize(&(point - self.center)); // apunta hacia afuera

        // Mapeo esferico: la normal se convierte en latitud/longitud.
        // Las UV solo hacen falta si hay una imagen que leer: las hadas, el
        // polvo y los cristales encendidos son de color liso, y el arco
        // tangente y el arco seno de cada impacto eran tiempo tirado.
        let (u, v) = match self.material.texture {
            crate::texture::Texture::Solid(_) => (0.0, 0.0),
            _ => (
                0.5 + normal.z.atan2(normal.x) / (2.0 * PI),
                0.5 - normal.y.asin() / PI,
            ),
        };

        Intersect::new(point, normal, distance, &self.material, u, v)
    }

    /// Lo que brilla solo no tapa.
    fn puede_tapar(&self) -> bool {
        self.material.emission_color.is_none()
    }

    /// La misma cuadratica, pero sin punto, sin normal, sin UV y sin
    /// clonar el material: solo si alguna de las dos raices cae entre el
    /// origen y la luz.
    fn occluded(&self, origin: &Vec3, direction: &Vec3, max_distance: f32) -> bool {
        if self.material.emission_color.is_some() {
            return false;
        }

        let oc = origin - self.center;

        let a = dot(direction, direction);
        let b = 2.0 * dot(direction, &oc);
        let c = dot(&oc, &oc) - self.radius * self.radius;

        let discriminant = b * b - 4.0 * a * c;
        if discriminant < 0.0 {
            return false;
        }

        let sqrt_d = discriminant.sqrt();
        let t1 = (-b - sqrt_d) / (2.0 * a);
        let t2 = (-b + sqrt_d) / (2.0 * a);

        (t1 > 0.001 && t1 < max_distance) || (t2 > 0.001 && t2 < max_distance)
    }

    /// La esfera ya ES su propio volumen acotante.
    fn bounds(&self) -> Option<(Vec3, f32)> {
        Some((self.center, self.radius))
    }
}
