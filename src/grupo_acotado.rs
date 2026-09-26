use crate::ray_intersect::{Intersect, RayIntersect};
use crate::vec3::Vec3;

/// Un puñado de objetos metidos adentro de una caja acotante.
///
/// La escena no tiene estructura de aceleracion: cada rayo prueba contra
/// TODOS los objetos, uno por uno. Eso se paga carisimo con piezas hechas
/// de muchos triangulos chiquitos y juntos (la espada son 41), porque un
/// rayo que pasa a metros de distancia igual las prueba una por una.
///
/// El grupo corta eso con una sola cuenta: si el rayo no toca la caja que
/// las contiene, no puede tocar ninguna, y las 41 pruebas se saltean de un
/// saque. Si la toca, recien ahi se prueba adentro y no se pierde nada.
///
/// El volumen NO se escribe a mano: sale de los propios hijos con
/// `GrupoAcotado::new`. Asi mover geometria no deja la caja corta, que es
/// el error que hace desaparecer piezas de la escena sin avisar.
pub struct GrupoAcotado {
    /// La CAJA que los contiene, no una esfera. Cambiado despues de medir:
    /// casi toda la geometria de esta escena son placas (losas del techo,
    /// molduras, peldaños) y para una placa la esfera es un envase
    /// pesimo. Ver el comentario de `aabb` en `ray_intersect.rs`.
    min: Vec3,
    max: Vec3,
    children: Vec<Box<dyn RayIntersect + Send + Sync>>,
}

impl GrupoAcotado {
    /// Recibe los hijos y calcula centro y radio solo.
    ///
    /// Si algun hijo no se puede acotar (un plano es infinito), el grupo se
    /// queda SIN volumen: el radio sale infinito y siempre se entra a probar
    /// adentro. Se pierde la optimizacion, pero nunca se pierde geometria.
    pub fn new(children: Vec<Box<dyn RayIntersect + Send + Sync>>) -> Self {
        Self::con_margen(children, 0.0)
    }

    /// Como `new`, pero con la caja acotante agrandada `margen` unidades.
    ///
    /// Es para los hijos que se MUEVEN despues de armada la escena (las
    /// hadas): la esfera se calcula una sola vez, asi que tiene que cubrir
    /// no solo donde nacieron sino hasta donde pueden llegar. Si un hijo se
    /// sale de la caja, los rayos que no la tocan lo dejan de ver.
    pub fn con_margen(children: Vec<Box<dyn RayIntersect + Send + Sync>>, margen: f32) -> Self {
        let (min, max) = Self::caja(&children);
        let m = Vec3::new(margen, margen, margen);

        GrupoAcotado {
            min: min - m,
            max: max + m,
            children,
        }
    }

    /// Vuelve a calcular la caja a partir de donde estan los hijos AHORA,
    /// con un margen.
    ///
    /// Es para los grupos cuyos hijos se mueven MUCHO y ademas se apagan:
    /// las estelas del arpa cruzan la escena entera, asi que la caja fija
    /// que las cubriria a todas es casi la escena entera, y entonces
    /// cualquier rayo que entra a la fuente termina probando las treinta
    /// esferas de las tres estelas aunque ninguna este encendida. Medido,
    /// eso costaba siete milisegundos por cuadro.
    ///
    /// Recalculandola cada cuadro, la caja queda pegada a la estela: la
    /// estela que esta cruzando se prueba solo si el rayo pasa cerca, y las
    /// ranuras apagadas (radio cero) quedan con una caja de tamano cero que
    /// no toca ningun rayo.
    pub fn recalcular_caja(&mut self, margen: f32) {
        let (min, max) = Self::caja(&self.children);
        let m = Vec3::new(margen, margen, margen);
        self.min = min - m;
        self.max = max + m;
    }

    /// Caja que contiene a todos los hijos.
    ///
    /// Si algun hijo no se puede acotar (un plano infinito), el grupo se
    /// queda SIN volumen: la caja sale infinita y siempre se entra a
    /// probar adentro. Se pierde la optimizacion, pero nunca se pierde
    /// geometria.
    fn caja(children: &[Box<dyn RayIntersect + Send + Sync>]) -> (Vec3, Vec3) {
        if children.is_empty() {
            // Un grupo vacio no puede tocar nada: caja degenerada y listo.
            return (Vec3::zeros(), Vec3::zeros());
        }

        let mut min = Vec3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY);
        let mut max = Vec3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);

        for child in children {
            let Some((a, b)) = child.aabb() else {
                return (
                    Vec3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY),
                    Vec3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY),
                );
            };
            for eje in 0..3 {
                min[eje] = min[eje].min(a[eje]);
                max[eje] = max[eje].max(b[eje]);
            }
        }

        (min, max)
    }

    /// Los hijos, para poder tocarlos despues de armada la escena: la
    /// animacion entra por aca a la gema de la espada, que vive adentro
    /// del grupo y no se alcanza desde la lista de objetos.
    ///
    /// OJO: mover un hijo de lugar NO recalcula la caja acotante. Sirve
    /// para cambiarle el material a una pieza, no para reubicarla.
    pub fn children_mut(&mut self) -> &mut [Box<dyn RayIntersect + Send + Sync>] {
        &mut self.children
    }

    /// Solo dice SI el rayo cruza la esfera acotante, no donde. Sirve para
    /// descartar, asi que no hace falta calcular el punto de impacto.
    ///
    /// Contempla que el origen este ADENTRO: en ese caso la raiz mas lejana
    /// es positiva aunque la mas cercana quede atras, y el rayo si cruza.
    /// El test de REBANADAS (slabs) contra la caja: cada eje define una
    /// franja del espacio y el rayo esta adentro solo mientras esta en las
    /// tres a la vez, asi que la entrada es el maximo de las tres entradas
    /// y la salida el minimo de las tres salidas.
    ///
    /// Se divide una sola vez por componente (`1 / direction`) y despues
    /// se multiplica, que es el truco de siempre para sacar las divisiones
    /// del bucle interno. Un componente en cero da infinito, y eso esta
    /// bien: un rayo paralelo a una rebanada queda "siempre adentro" o
    /// "siempre afuera" sin caso especial.
    fn hits_bounds(&self, origin: &Vec3, direction: &Vec3) -> bool {
        let mut t_entra = 0.0f32;
        let mut t_sale = f32::INFINITY;

        for eje in 0..3 {
            let inv = 1.0 / direction[eje];
            let mut a = (self.min[eje] - origin[eje]) * inv;
            let mut b = (self.max[eje] - origin[eje]) * inv;
            if a > b {
                std::mem::swap(&mut a, &mut b);
            }
            if a > t_entra {
                t_entra = a;
            }
            if b < t_sale {
                t_sale = b;
            }
            if t_entra > t_sale {
                return false;
            }
        }

        t_sale > 1e-3
    }
}

impl RayIntersect for GrupoAcotado {
    fn ray_intersect<'a>(&'a self, origin: &Vec3, direction: &Vec3) -> Intersect<'a> {
        if !self.hits_bounds(origin, direction) {
            return Intersect::empty();
        }

        // Adentro del grupo se hace lo mismo que en el bucle principal de la
        // escena: gana el impacto de menor t positiva.
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

    /// Un grupo tapa solo si alguno de sus hijos puede. Los dos anillos son
    /// todo esferas emisivas, asi que sus nueve grupos se van enteros del
    /// bucle de sombras: nueve cuadraticas menos por luz y por impacto.
    fn puede_tapar(&self) -> bool {
        self.children.iter().any(|hijo| hijo.puede_tapar())
    }

    /// Para tapar la luz alcanza con UN hijo: se sale con el primero que
    /// lo haga, sin mirar los demas. `any` ya corta solo.
    ///
    /// Esto ademas ARREGLA algo. Por `ray_intersect` el grupo devuelve un
    /// solo impacto, el mas cercano, y el rayo de sombra descarta lo que
    /// brilla solo: si el farol emisivo quedaba delante de su columna, el
    /// grupo devolvia el farol, el farol se descartaba, y la columna no se
    /// probaba nunca. Esa columna dejaba de dar sombra. Aca cada hijo
    /// responde por su cuenta y el farol no tapa a nadie.
    fn occluded(&self, origin: &Vec3, direction: &Vec3, max_distance: f32) -> bool {
        if !self.hits_bounds(origin, direction) {
            return false;
        }

        self.children
            .iter()
            .any(|child| child.occluded(origin, direction, max_distance))
    }

    /// Lo que deja pasar el grupo es el PRODUCTO de lo que deja pasar cada
    /// hijo en el camino: dos caras de cristal atenuan dos veces. Se corta
    /// en cuanto alguno tapa del todo.
    fn transmision(&self, origin: &Vec3, direction: &Vec3, max_distance: f32) -> f32 {
        if !self.hits_bounds(origin, direction) {
            return 1.0;
        }

        let mut pasa = 1.0f32;
        for child in &self.children {
            pasa *= child.transmision(origin, direction, max_distance);
            if pasa <= 0.0 {
                return 0.0;
            }
        }
        pasa
    }

    /// Un grupo se acota como cualquier otra primitiva, asi que se pueden
    /// meter grupos adentro de grupos y el de afuera calcula su volumen solo.
    fn aabb(&self) -> Option<(Vec3, Vec3)> {
        if self.min.x.is_finite() && self.max.x.is_finite() {
            Some((self.min, self.max))
        } else {
            None
        }
    }

    /// La esfera que envuelve a la caja, para quien todavia pida esferas.
    fn bounds(&self) -> Option<(Vec3, f32)> {
        self.aabb().map(|(min, max)| {
            let centro = (min + max) * 0.5;
            (centro, ((max - min) * 0.5).norm())
        })
    }
}


