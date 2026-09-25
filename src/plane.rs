use crate::material::Material;
use crate::ray_intersect::{Intersect, RayIntersect};
use nalgebra_glm::{dot, normalize, Vec3};

/// Hasta donde llega un plano con limite, alrededor de su `point`.
#[derive(Clone, Copy, Debug)]
pub enum Limite {
    /// Semiejes (en x, en z) de una elipse. La fuente no lo usa (su piscina
    /// es cuadrada), pero se queda: es parte del engine.
    #[allow(dead_code)]
    Elipse(f32, f32),
    /// Medios lados (en x, en z) de un rectangulo.
    Rectangulo(f32, f32),
}

impl Limite {
    /// Si el corrimiento `(dx, dz)` desde el centro cae adentro.
    fn contiene(self, dx: f32, dz: f32) -> bool {
        match self {
            Limite::Elipse(a, b) => {
                let (ex, ez) = (dx / a, dz / b);
                ex * ex + ez * ez <= 1.0
            }
            Limite::Rectangulo(a, b) => dx.abs() <= a && dz.abs() <= b,
        }
    }

    /// Radio de la esfera que lo cubre.
    fn radio(self) -> f32 {
        match self {
            Limite::Elipse(a, b) => a.max(b),
            Limite::Rectangulo(a, b) => (a * a + b * b).sqrt(),
        }
    }
}

/// Plano definido por un punto y su normal. Infinito, salvo que tenga
/// `limite`. Para el agua de la fuente: point a la altura del agua, normal
/// apuntando arriba.
pub struct Plane {
    pub point: Vec3,
    pub normal: Vec3,
    /// Centro de los anillos de la onda (solo importan X y Z).
    pub ripple_center: Vec3,
    /// Amplitud de las ondas. En 0 el plano queda liso, como antes.
    ///
    /// La geometria NO se deforma: lo que se inclina es la NORMAL, que es
    /// lo unico que mira el sombreado. Sale mucho mas barato que subdividir
    /// el agua y, mientras las olas sean bajitas, se ve igual: el reflejo
    /// se quiebra, el especular se rompe en chispas y la Trifuerza tiembla
    /// abajo del agua.
    pub ripple_strength: f32,
    /// Que tan juntos salen los anillos.
    pub ripple_scale: f32,
    /// Fase del oleaje, en radianes. Avanzandola con el tiempo los anillos
    /// VIAJAN desde el centro hacia afuera en vez de quedarse congelados:
    /// la animacion pide esto, y no la geometria, en cada cuadro.
    pub ripple_phase: f32,
    /// Cuantas repeticiones de la textura entran en una unidad del mundo.
    ///
    /// Antes estaba clavado en 2.0, o sea una repeticion cada media unidad.
    /// Para el agua servia (las UV densas se leian como ondas), pero para
    /// una baldosa de escenario eso es una cuadricula de medio metro que de
    /// lejos se ve como ruido. Con 0.25 la baldosa mide 4 unidades.
    pub uv_scale: f32,
    /// Hasta donde llega el plano, centrado en `point`. En `None` el plano
    /// es infinito, como siempre.
    ///
    /// Es lo que hace posible el agua de la piscina: un plano infinito a
    /// la altura del agua taparia la cueva entera. Fuera del limite el rayo
    /// pasa de largo.
    pub limite: Option<Limite>,
    pub material: Material,
}

impl Plane {
    /// La t del impacto contra el plano infinito, si lo hay adelante del
    /// rayo y adentro del limite.
    fn t_impacto(&self, origin: &Vec3, direction: &Vec3) -> Option<f32> {
        let denom = dot(&self.normal, direction);

        // Si denom es ~0, el rayo es paralelo al plano: no hay impacto.
        if denom.abs() < 1e-6 {
            return None;
        }

        let t = dot(&(self.point - origin), &self.normal) / denom;
        if t <= 1e-3 {
            return None;
        }

        if let Some(limite) = self.limite {
            let p = origin + direction * t;
            if !limite.contiene(p.x - self.point.x, p.z - self.point.z) {
                return None;
            }
        }

        Some(t)
    }

    /// Normal inclinada por el oleaje, en el punto dado.
    ///
    /// La altura de la ola es la suma de dos ondas: uno son los anillos
    /// que salen del centro de la fuente y la otra una onda cruzada que
    /// los desordena, para que el agua no parezca un blanco de tiro.
    /// La normal sale de la pendiente de esa suma.
    ///
    /// Asume que el plano es horizontal (que es el caso del agua): la
    /// pendiente se arma directo sobre X y Z.
    fn rippled_normal(&self, point: &Vec3) -> Vec3 {
        if self.ripple_strength <= 0.0 {
            return self.normal;
        }

        let dx = point.x - self.ripple_center.x;
        let dz = point.z - self.ripple_center.z;
        // El maximo evita dividir entre cero justo en el centro.
        let distance = (dx * dx + dz * dz).sqrt().max(1e-4);

        let k = self.ripple_scale;

        // 1. Anillos concentricos: altura = sin(distancia * k - fase).
        //    Su pendiente apunta radialmente hacia afuera. Restar la fase
        //    hace que los anillos se alejen del centro con el tiempo.
        let radial = k * (distance * k - self.ripple_phase).cos();
        let mut slope_x = radial * dx / distance;
        let mut slope_z = radial * dz / distance;

        // 2. Onda cruzada, mas corta y mas suave, en diagonal.
        let cross = 0.30 * k * 1.7;
        let phase = (point.x * k * 1.7 + point.z * k * 1.1 + self.ripple_phase * 0.6).cos();
        slope_x += cross * phase;
        slope_z += cross * phase * 0.65;

        // La normal de una superficie de altura h(x, z) es (-dh/dx, 1, -dh/dz).
        normalize(&Vec3::new(
            -slope_x * self.ripple_strength,
            1.0,
            -slope_z * self.ripple_strength,
        ))
    }
}

impl RayIntersect for Plane {
    /// Lo que brilla solo no tapa.
    fn puede_tapar(&self) -> bool {
        self.material.emission_color.is_none()
    }

    /// El plano no necesita la normal ni las UV para saber si tapa: alcanza
    /// con la t. Sin oleaje tampoco, que igual solo inclina la normal.
    fn occluded(&self, origin: &Vec3, direction: &Vec3, max_distance: f32) -> bool {
        if self.material.emission_color.is_some() {
            return false;
        }

        matches!(self.t_impacto(origin, direction), Some(t) if t < max_distance)
    }

    /// El agua deja pasar la luz: lo que atraviesa es su peso de
    /// refraccion, asi que el fondo de la piscina se ilumina a traves del
    /// agua en vez de quedar en sombra negra.
    fn transmision(&self, origin: &Vec3, direction: &Vec3, max_distance: f32) -> f32 {
        if !self.occluded(origin, direction, max_distance) {
            return 1.0;
        }
        self.material.albedo[3]
    }

    /// Un plano horizontal con limite es una placa FINA: su caja mide lo
    /// que el limite en X y Z y casi nada en Y. La esfera de `bounds`, en
    /// cambio, tiene el radio de la diagonal del limite (4.1 para el agua
    /// de la piscina) y se come toda la fuente.
    fn aabb(&self) -> Option<(Vec3, Vec3)> {
        self.limite.map(|l| {
            let (a, b) = match l {
                Limite::Elipse(a, b) | Limite::Rectangulo(a, b) => (a, b),
            };
            // Un pelo de espesor en Y: la caja no puede ser degenerada o
            // el test de rebanadas se queda sin volumen que cruzar.
            (
                self.point - Vec3::new(a, 0.01, b),
                self.point + Vec3::new(a, 0.01, b),
            )
        })
    }

    /// Un plano con limite si se puede acotar: la esfera que cubre el
    /// limite. Sin limite es infinito y no.
    fn bounds(&self) -> Option<(Vec3, f32)> {
        self.limite.map(|l| (self.point, l.radio()))
    }

    fn ray_intersect<'a>(&'a self, origin: &Vec3, direction: &Vec3) -> Intersect<'a> {
        if let Some(t) = self.t_impacto(origin, direction) {
            let hit_point = origin + direction * t;

            // El plano es infinito, asi que el patron se repite: se toma
            // solo la parte fraccionaria. El 2.0 es la escala del mosaico:
            // mas alto = repeticiones mas chicas y juntas, que en el agua
            // se leen como ondas.
            let mut u = (hit_point.x * self.uv_scale).fract();
            let mut v = (hit_point.z * self.uv_scale).fract();
            if u < 0.0 {
                u += 1.0;
            }
            if v < 0.0 {
                v += 1.0;
            }

            let normal = self.rippled_normal(&hit_point);

            Intersect::new(hit_point, normal, t, &self.material, u, v)
        } else {
            Intersect::empty()
        }
    }
}
