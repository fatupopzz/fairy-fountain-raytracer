use crate::material::Material;
use crate::ray_intersect::{Intersect, RayIntersect};
use nalgebra_glm::{cross, dot, normalize, Vec3};

/// Triangulo definido por sus tres vertices.
/// Con esto ya se puede armar cualquier figura plana pegando triangulos.
pub struct Triangle {
    pub a: Vec3,
    pub b: Vec3,
    pub c: Vec3,
    /// UV propias de cada vertice. Las baricentricas sirven para un
    /// degrade, pero deforman cualquier textura con cuadricula; con
    /// estas se controla como se estira la imagen sobre el triangulo.
    /// Si van en None se usan las baricentricas de siempre.
    pub uv_a: Option<(f32, f32)>,
    pub uv_b: Option<(f32, f32)>,
    pub uv_c: Option<(f32, f32)>,
    pub material: Material,
}

impl RayIntersect for Triangle {
    fn ray_intersect<'a>(&'a self, origin: &Vec3, direction: &Vec3) -> Intersect<'a> {
        // Moller-Trumbore: en vez de intersectar contra el plano y despues
        // ver si cae adentro, saca t y las coordenadas baricentricas (u, v)
        // de una sola vez.
        let edge1 = self.b - self.a;
        let edge2 = self.c - self.a;

        let p = cross(direction, &edge2);
        let det = dot(&edge1, &p);

        // det ~ 0 quiere decir que el rayo va paralelo al triangulo.
        // El epsilon es mas fino que el de las otras primitivas porque
        // un triangulo es plano y pierde precision mucho mas rapido.
        if det.abs() < 1e-6 {
            return Intersect::empty();
        }

        let inv_det = 1.0 / det;
        let tvec = origin - self.a;

        // u y v son las coordenadas baricentricas del impacto. Si alguna
        // se sale de [0, 1] (o si u + v pasa de 1), el rayo le pego al
        // plano pero por fuera del triangulo.
        let u = dot(&tvec, &p) * inv_det;
        if !(0.0..=1.0).contains(&u) {
            return Intersect::empty();
        }

        let q = cross(&tvec, &edge1);
        let v = dot(direction, &q) * inv_det;
        if v < 0.0 || u + v > 1.0 {
            return Intersect::empty();
        }

        let t = dot(&edge2, &q) * inv_det;

        // Igual que en las demas primitivas: solo lo que esta adelante,
        // con epsilon para no re-intersectar la cara de la que se sale.
        if t <= 1e-3 {
            return Intersect::empty();
        }

        let point = origin + direction * t;
        let normal = normalize(&cross(&edge1, &edge2));

        // Si el triangulo trae UV propias, se interpolan con las
        // baricentricas; si no, las baricentricas mismas hacen de UV.
        let (tex_u, tex_v) = match (self.uv_a, self.uv_b, self.uv_c) {
            (Some(uv_a), Some(uv_b), Some(uv_c)) => {
                let w = 1.0 - u - v; // peso del vertice A
                (
                    uv_a.0 * w + uv_b.0 * u + uv_c.0 * v,
                    uv_a.1 * w + uv_b.1 * u + uv_c.1 * v,
                )
            }
            _ => (u, v),
        };

        Intersect::new(point, normal, t, &self.material, tex_u, tex_v)
    }

    /// Lo que brilla solo no tapa.
    fn puede_tapar(&self) -> bool {
        self.material.emission_color.is_none()
    }

    /// Moller-Trumbore igual que arriba, pero cortando apenas se sabe la
    /// respuesta: no calcula el punto, ni la normal, ni interpola UV, ni
    /// clona el material. Es la primitiva que mas veces se prueba de toda
    /// la escena (la espada sola son 41), asi que es donde mas rinde.
    fn occluded(&self, origin: &Vec3, direction: &Vec3, max_distance: f32) -> bool {
        if self.material.emission_color.is_some() {
            return false;
        }

        let edge1 = self.b - self.a;
        let edge2 = self.c - self.a;

        let p = cross(direction, &edge2);
        let det = dot(&edge1, &p);

        if det.abs() < 1e-6 {
            return false;
        }

        let inv_det = 1.0 / det;
        let tvec = origin - self.a;

        let u = dot(&tvec, &p) * inv_det;
        if !(0.0..=1.0).contains(&u) {
            return false;
        }

        let q = cross(&tvec, &edge1);
        let v = dot(direction, &q) * inv_det;
        if v < 0.0 || u + v > 1.0 {
            return false;
        }

        let t = dot(&edge2, &q) * inv_det;
        t > 1e-3 && t < max_distance
    }

    /// Las caras de cristal dejan pasar su peso de refraccion.
    fn transmision(&self, origin: &Vec3, direction: &Vec3, max_distance: f32) -> f32 {
        if !self.occluded(origin, direction, max_distance) {
            return 1.0;
        }
        self.material.albedo[3]
    }

    /// La caja de los tres vertices, exacta. Contra la esfera del
    /// baricentro la diferencia es grande en los triangulos flacos, que
    /// son casi todos los de la Trifuerza y las rupias.
    fn aabb(&self) -> Option<(Vec3, Vec3)> {
        let min = Vec3::new(
            self.a.x.min(self.b.x).min(self.c.x),
            self.a.y.min(self.b.y).min(self.c.y),
            self.a.z.min(self.b.z).min(self.c.z),
        );
        let max = Vec3::new(
            self.a.x.max(self.b.x).max(self.c.x),
            self.a.y.max(self.b.y).max(self.c.y),
            self.a.z.max(self.b.z).max(self.c.z),
        );
        Some((min, max))
    }

    /// Centro en el baricentro y radio hasta el vertice mas lejano. No es la
    /// esfera mas chica posible, pero se saca de tres restas y contiene al
    /// triangulo entero, que es lo unico que no se puede fallar.
    fn bounds(&self) -> Option<(Vec3, f32)> {
        let center = (self.a + self.b + self.c) / 3.0;

        let radius = (self.a - center)
            .magnitude()
            .max((self.b - center).magnitude())
            .max((self.c - center).magnitude());

        Some((center, radius))
    }
}
