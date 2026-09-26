use crate::material::Material;
use crate::texture::Texture;
use crate::vec3::Vec3;
use raylib::prelude::Color;
use std::any::Any;

/// El material del "no le pego a nada". Es una constante global para que
/// `Intersect::empty` pueda PRESTARLO igual que las primitivas prestan el
/// suyo, sin construir nada.
static NEGRO: Material = Material {
    albedo: [0.0; 4],
    specular: 0.0,
    refractive_index: 0.0,
    texture: Texture::Solid(Color::new(0, 0, 0, 255)),
    emission_color: None,
    relieve: None,
    rugosidad: 0.0,
    causticas: 0.0,
};

/// Todo lo que el sombreado necesita saber de un impacto.
/// Antes bastaba con la distancia; para que la esfera se VEA esfera
/// hace falta la normal.
///
/// El material va PRESTADO de la primitiva, no clonado. Clonarlo era tocar
/// el contador del `Arc` de la textura (una escritura atomica) en cada
/// impacto de cada rayo, con todos los nucleos peleandose la misma linea
/// de cache; prestado no cuesta nada, y el impacto vive menos que la
/// primitiva de todos modos.
pub struct Intersect<'a> {
    pub distance: f32,
    pub point: Vec3,
    pub normal: Vec3,
    pub material: &'a Material,
    /// Coordenadas de textura del impacto, en [0, 1].
    pub u: f32,
    pub v: f32,
    pub is_intersecting: bool,
}

impl<'a> Intersect<'a> {
    pub fn new(
        point: Vec3,
        normal: Vec3,
        distance: f32,
        material: &'a Material,
        u: f32,
        v: f32,
    ) -> Self {
        Intersect {
            distance,
            point,
            normal,
            material,
            u,
            v,
            is_intersecting: true,
        }
    }

    /// "No te toque". Se usa cuando el rayo pasa de largo.
    pub fn empty() -> Self {
        Intersect {
            distance: 0.0,
            point: Vec3::zeros(),
            normal: Vec3::zeros(),
            material: &NEGRO,
            u: 0.0,
            v: 0.0,
            is_intersecting: false,
        }
    }
}

/// Todas las primitivas responden lo mismo. Cuando agregues plano
/// y triangulo, solo implementan este trait y el resto no cambia.
///
/// Pide `Any` de supertrait por una sola razon: la animacion necesita
/// volver a tocar una primitiva concreta (mover una esfera, cambiarle la
/// emision a un triangulo) y por el trait objeto no se llega. Con esto un
/// `&mut dyn RayIntersect` se puede subir a `&mut dyn Any` y de ahi bajar
/// al tipo real. El trazado no lo usa para nada.
pub trait RayIntersect: Any {
    fn ray_intersect<'a>(&'a self, origin: &Vec3, direction: &Vec3) -> Intersect<'a>;

    /// ¿Este objeto TAPA la luz entre `origin` y algo a `max_distance`?
    ///
    /// Es la pregunta que hace un rayo de sombra, y es mucho mas floja que
    /// la de `ray_intersect`: no importa donde pego, ni con que normal, ni
    /// que UV, ni de que material es. Solo si tapa o no.
    ///
    /// Eso cambia el costo por completo. `ray_intersect` termina armando un
    /// `Intersect` entero, y ahi adentro hay un `material.clone()`, que en
    /// una textura de imagen es tocar el contador de un `Arc`: una escritura
    /// atomica. Los rayos de sombra son la mayor parte del trabajo de la
    /// escena (nueve luces, y cada impacto prueba TODOS los objetos contra
    /// cada una), asi que eso son millones de atomicas por cuadro, y todos
    /// los nucleos peleandose la misma linea de cache.
    ///
    /// Lo que brilla solo NO tapa: una mota de hada pegada al techo no
    /// tiene que estamparle un manchon negro a lo que hay abajo.
    ///
    /// La version de aca es la segura y la lenta, y sirve para cualquier
    /// primitiva nueva sin tocar nada. Las que se prueban muchas veces la
    /// pisan con una cuenta geometrica pelada.
    fn occluded(&self, origin: &Vec3, direction: &Vec3, max_distance: f32) -> bool {
        let hit = self.ray_intersect(origin, direction);

        hit.is_intersecting
            && hit.distance < max_distance
            && hit.material.emission_color.is_none()
    }

    /// Cuanta luz DEJA PASAR este objeto entre `origin` y la luz, de 0
    /// (tapa del todo) a 1 (no esta en el medio).
    ///
    /// Es `occluded` con un matiz: lo transparente no hace sombra negra.
    /// El agua y el cristal dejan pasar una parte de la luz (su peso de
    /// refraccion), asi que el fondo de la piscina se ve iluminado a traves
    /// del agua y un cristal proyecta una sombra tenue y no un manchon.
    /// La version por defecto es la de siempre: tapa o no tapa.
    fn transmision(&self, origin: &Vec3, direction: &Vec3, max_distance: f32) -> f32 {
        if self.occluded(origin, direction, max_distance) {
            0.0
        } else {
            1.0
        }
    }

    /// ¿Este objeto puede tapar la luz de ALGUNA manera?
    ///
    /// Se pregunta UNA vez, al armar la escena, para dejar de lado a los que
    /// nunca van a dar sombra. En este escenario eso es casi todo: de los
    /// cuarenta y pico de objetos, cuarenta y cuatro son emisivos (las
    /// esferas de los dos anillos y los haces de laser) y por definicion no
    /// tapan a nadie. Los que quedan son siete: el piso, los cuatro
    /// soportes, la esfera de cristal y la pared.
    ///
    /// La diferencia es grande porque los rayos de sombra son la mayor parte
    /// del trabajo: seis luces por cada punto que se sombrea, y cada uno
    /// probando la lista entera. Preguntar por adelantado quien puede tapar
    /// saca del bucle no solo a los emisivos sino tambien a las esferas
    /// acotantes que los envuelven, que son una cuadratica cada una.
    fn puede_tapar(&self) -> bool {
        true
    }

    /// CAJA alineada a los ejes que envuelve a la primitiva: esquina
    /// minima y maxima.
    ///
    /// Existe ademas de `bounds` porque para casi toda la geometria de
    /// esta escena la esfera es un envase pesimo. El piso mide 30 x 1 x
    /// 30: su caja es exacta, y su esfera tiene radio 21 y cubre la cueva
    /// entera, con lo que practicamente ningun rayo la puede descartar.
    /// Lo mismo las paredes, las losas del techo y las molduras, que son
    /// todas placas finas y anchas.
    ///
    /// Por defecto se deriva de `bounds`, asi que una primitiva nueva
    /// funciona sin escribir nada; las que tienen una caja exacta (el
    /// cubo, el triangulo) la dan directamente.
    fn aabb(&self) -> Option<(Vec3, Vec3)> {
        self.bounds().map(|(c, r)| {
            (
                c - Vec3::new(r, r, r),
                c + Vec3::new(r, r, r),
            )
        })
    }

    /// Esfera que envuelve a la primitiva: centro y radio.
    ///
    /// Con esto un grupo saca su volumen acotante solo, sin que haya que
    /// darle las medidas a mano cada vez que se mueve la geometria.
    ///
    /// `None` significa "no se puede acotar": es el caso del plano, que es
    /// infinito. Un grupo que reciba un hijo asi se queda sin volumen y
    /// prueba a todos sus hijos siempre, que es lo unico seguro.
    fn bounds(&self) -> Option<(Vec3, f32)> {
        None
    }
}
