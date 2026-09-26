use crate::material::Material;
use crate::ray_intersect::{Intersect, RayIntersect};
use crate::vec3::Vec3;

/// Cubo alineado a los ejes (AABB), estilo Minecraft.
///
/// No rota: la escena se arma apilando cubos alineados, y esa restriccion
/// es justo lo que lo hace barato. Un rayo contra un cubo alineado son seis
/// divisiones y un punado de comparaciones, sin matrices ni productos.
pub struct Cube {
    /// Esquina minima (x_min, y_min, z_min).
    pub min: Vec3,
    /// Esquina maxima (x_max, y_max, z_max).
    pub max: Vec3,
    /// Cuantas repeticiones de la textura entran en UNA unidad del mundo.
    ///
    /// En 0 (el valor por defecto) la textura se estira una sola vez sobre
    /// cada cara, que para un bloque chico esta bien pero para el piso de
    /// 30 unidades es una piedra gigante y borrosa. Con 0.25 la textura se
    /// repite cada cuatro unidades y el piso se lee como piso.
    pub tile: f32,
    pub material: Material,
}

/// Un rayo que arranca a menos de esto de una cara no cuenta como impacto:
/// es el mismo epsilon de las demas primitivas, para que un rayo secundario
/// no se re-intersecte con la superficie de la que acaba de salir.
const EPSILON: f32 = 0.001;

impl Cube {
    /// Cubo de lado `size` centrado en `center`.
    pub fn new(center: Vec3, size: f32, material: Material) -> Self {
        Self::new_rect(center, size, size, size, material)
    }

    /// Cuboide con un tamanio distinto por eje, centrado en `center`.
    pub fn new_rect(center: Vec3, size_x: f32, size_y: f32, size_z: f32, material: Material) -> Self {
        let half = Vec3::new(size_x / 2.0, size_y / 2.0, size_z / 2.0);

        Cube {
            min: center - half,
            max: center + half,
            tile: 0.0,
            material,
        }
    }

    /// El mismo cubo con la textura repetida cada `1 / por_unidad` unidades
    /// en vez de estirada sobre la cara.
    pub fn con_mosaico(mut self, por_unidad: f32) -> Self {
        self.tile = por_unidad;
        self
    }

    /// El metodo de slabs: la parte geometrica pelada, compartida entre el
    /// impacto completo y el rayo de sombra.
    ///
    /// Cada eje define una "rebanada" del espacio entre sus dos planos, y el
    /// rayo esta dentro del cubo solo mientras esta dentro de las tres a la
    /// vez. Asi que la entrada es el MAXIMO de las tres entradas y la salida
    /// el MINIMO de las tres salidas.
    ///
    /// Devuelve `(t, eje)`: la distancia del impacto y cual eje (0 = X,
    /// 1 = Y, 2 = Z) fue el que decidio la entrada, que es el que da la
    /// normal. Si el rayo arranca ADENTRO del cubo, el impacto es la salida:
    /// es lo que hace que la refraccion funcione cuando el rayo sale.
    ///
    /// Una componente cero de la direccion divide por cero y da `inf` (o
    /// `-inf`), y eso esta bien: `inf` compara correctamente contra
    /// cualquier numero, asi que un rayo paralelo a una rebanada la deja
    /// "siempre adentro" o "siempre afuera" sin un caso especial. Lo unico
    /// que hay que evitar es un `0.0 / 0.0`, que daria NaN; solo pasa si el
    /// origen esta EXACTAMENTE sobre un plano del cubo con direccion
    /// paralela, y el `max`/`min` de abajo lo descarta porque NaN pierde
    /// contra todo.
    fn slabs(&self, origin: &Vec3, direction: &Vec3) -> Option<(f32, usize)> {
        let mut t_entry = f32::NEG_INFINITY;
        let mut t_exit = f32::INFINITY;
        let mut eje_entrada = 0;

        for eje in 0..3 {
            let inv = 1.0 / direction[eje];
            let mut t_min = (self.min[eje] - origin[eje]) * inv;
            let mut t_max = (self.max[eje] - origin[eje]) * inv;

            if t_min > t_max {
                std::mem::swap(&mut t_min, &mut t_max);
            }

            // `>` y no `max()`: NaN nunca gana la comparacion, y ademas asi
            // el eje que se queda anotado es el ultimo que subio la entrada.
            if t_min > t_entry {
                t_entry = t_min;
                eje_entrada = eje;
            }
            if t_max < t_exit {
                t_exit = t_max;
            }
        }

        // Las rebanadas no se cruzan, o el cubo entero quedo atras.
        if t_entry >= t_exit || t_exit <= EPSILON {
            return None;
        }

        if t_entry > EPSILON {
            Some((t_entry, eje_entrada))
        } else {
            // El origen esta adentro: el impacto es la cara de salida. El eje
            // de la normal se recalcula abajo a partir del punto, porque
            // `eje_entrada` describe la cara por la que se ENTRO.
            Some((t_exit, 3))
        }
    }

    /// Con que eje coincide la cara del cubo en la que cae `point`: la que
    /// esta mas cerca, medida como distancia a sus dos planos.
    fn eje_de_la_cara(&self, point: &Vec3) -> usize {
        let mut mejor = 0;
        let mut menor = f32::INFINITY;

        for eje in 0..3 {
            let d = (point[eje] - self.min[eje])
                .abs()
                .min((point[eje] - self.max[eje]).abs());

            if d < menor {
                menor = d;
                mejor = eje;
            }
        }

        mejor
    }
}

impl RayIntersect for Cube {
    fn ray_intersect<'a>(&'a self, origin: &Vec3, direction: &Vec3) -> Intersect<'a> {
        let Some((distance, eje)) = self.slabs(origin, direction) else {
            return Intersect::empty();
        };

        let point = origin + direction * distance;

        let eje = if eje == 3 { self.eje_de_la_cara(&point) } else { eje };

        // La normal apunta HACIA el rayo, no en la direccion del eje: si el
        // rayo viaja en +X pego en la cara de x_min, cuya normal es -X.
        let mut normal = Vec3::zeros();
        normal[eje] = -direction[eje].signum();

        // UV por cara, en [0, 1] sobre la cara golpeada: los dos ejes que NO
        // son el de la normal recorren la cara.
        let extent = self.max - self.min;
        let local = point - self.min;
        let (u, v) = match eje {
            0 => (local.z / extent.z, local.y / extent.y),
            1 => (local.x / extent.x, local.z / extent.z),
            _ => (local.x / extent.x, local.y / extent.y),
        };

        // Con mosaico las UV van en unidades del mundo (la textura repite
        // sola con `fract`); sin el, una copia por cara.
        let (u, v) = if self.tile > 0.0 {
            match eje {
                0 => (local.z * self.tile, local.y * self.tile),
                1 => (local.x * self.tile, local.z * self.tile),
                _ => (local.x * self.tile, local.y * self.tile),
            }
        } else {
            (u.clamp(0.0, 1.0), v.clamp(0.0, 1.0))
        };

        Intersect::new(point, normal, distance, &self.material, u, v)
    }

    /// Lo que brilla solo no tapa.
    fn puede_tapar(&self) -> bool {
        self.material.emission_color.is_none()
    }

    /// Los mismos slabs, pero sin punto, sin normal, sin UV y sin clonar el
    /// material: solo si el impacto cae entre el origen y la luz.
    fn occluded(&self, origin: &Vec3, direction: &Vec3, max_distance: f32) -> bool {
        if self.material.emission_color.is_some() {
            return false;
        }

        matches!(self.slabs(origin, direction), Some((t, _)) if t < max_distance)
    }

    /// Un cubo de cristal deja pasar parte de la luz (su peso de
    /// refraccion): sombra tenue, no negra.
    fn transmision(&self, origin: &Vec3, direction: &Vec3, max_distance: f32) -> f32 {
        if !self.occluded(origin, direction, max_distance) {
            return 1.0;
        }
        self.material.albedo[3]
    }

    /// La caja del cubo ES el cubo: no hay envase mas ajustado.
    fn aabb(&self) -> Option<(Vec3, Vec3)> {
        Some((self.min, self.max))
    }

    /// La esfera que pasa por las ocho esquinas: centro del cubo y media
    /// diagonal.
    fn bounds(&self) -> Option<(Vec3, f32)> {
        let center = (self.min + self.max) * 0.5;
        let radius = ((self.max - self.min) * 0.5).norm();

        Some((center, radius))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::texture::Texture;
    use raylib::prelude::Color;

    fn cubo() -> Cube {
        let material = Material::new(
            [1.0, 0.0, 0.0, 0.0],
            1.0,
            0.0,
            Texture::Solid(Color::new(255, 255, 255, 255)),
            None,
        );
        Cube::new(Vec3::new(0.0, 0.0, 0.0), 2.0, material)
    }

    #[test]
    fn el_constructor_centra_el_cubo() {
        let c = cubo();
        assert_eq!(c.min, Vec3::new(-1.0, -1.0, -1.0));
        assert_eq!(c.max, Vec3::new(1.0, 1.0, 1.0));

        let r = Cube::new_rect(Vec3::new(1.0, 2.0, 3.0), 2.0, 4.0, 6.0, c.material.clone());
        assert_eq!(r.min, Vec3::new(0.0, 0.0, 0.0));
        assert_eq!(r.max, Vec3::new(2.0, 4.0, 6.0));
    }

    #[test]
    fn pega_de_frente_con_la_normal_hacia_el_rayo() {
        let c = cubo();
        let hit = c.ray_intersect(&Vec3::new(0.0, 0.0, -5.0), &Vec3::new(0.0, 0.0, 1.0));

        assert!(hit.is_intersecting);
        assert!((hit.distance - 4.0).abs() < 1e-5);
        assert_eq!(hit.normal, Vec3::new(0.0, 0.0, -1.0));
        // Centro de la cara Z: u y v a la mitad.
        assert!((hit.u - 0.5).abs() < 1e-5 && (hit.v - 0.5).abs() < 1e-5);
    }

    #[test]
    fn la_normal_sigue_a_la_cara_de_entrada() {
        let c = cubo();

        let desde_x = c.ray_intersect(&Vec3::new(5.0, 0.2, 0.3), &Vec3::new(-1.0, 0.0, 0.0));
        assert_eq!(desde_x.normal, Vec3::new(1.0, 0.0, 0.0));
        // Cara X: u recorre Z y v recorre Y.
        assert!((desde_x.u - 0.65).abs() < 1e-5 && (desde_x.v - 0.6).abs() < 1e-5);

        let desde_arriba = c.ray_intersect(&Vec3::new(0.0, 5.0, 0.0), &Vec3::new(0.0, -1.0, 0.0));
        assert_eq!(desde_arriba.normal, Vec3::new(0.0, 1.0, 0.0));
    }

    #[test]
    fn pasa_de_largo_o_queda_atras() {
        let c = cubo();

        // Paralelo a Z pero corrido en X: la direccion tiene ceros y aun
        // asi no hay impacto ni NaN.
        let al_lado = c.ray_intersect(&Vec3::new(3.0, 0.0, -5.0), &Vec3::new(0.0, 0.0, 1.0));
        assert!(!al_lado.is_intersecting);

        // El cubo entero queda detras del origen.
        let atras = c.ray_intersect(&Vec3::new(0.0, 0.0, 5.0), &Vec3::new(0.0, 0.0, 1.0));
        assert!(!atras.is_intersecting);
    }

    #[test]
    fn desde_adentro_pega_en_la_salida() {
        let c = cubo();
        let hit = c.ray_intersect(&Vec3::new(0.0, 0.0, 0.0), &Vec3::new(0.0, 0.0, 1.0));

        assert!(hit.is_intersecting);
        assert!((hit.distance - 1.0).abs() < 1e-5);
        assert_eq!(hit.normal, Vec3::new(0.0, 0.0, -1.0));
    }

    #[test]
    fn tapa_la_luz_solo_si_esta_en_el_medio() {
        let c = cubo();
        let origen = Vec3::new(0.0, 0.0, -5.0);
        let dir = Vec3::new(0.0, 0.0, 1.0);

        assert!(c.occluded(&origen, &dir, 10.0));
        assert!(!c.occluded(&origen, &dir, 3.0), "la luz esta antes del cubo");
    }
}
