//! EL TORO: un anillo, y la unica figura de esta escena cuya interseccion
//! no sale de una cuadratica.
//!
//! Todo lo demas que hay aca —la esfera, el cilindro, el plano, el cubo,
//! el triangulo— se resuelve despejando una ecuacion de segundo grado o
//! menos. El toro no: sustituyendo el rayo en su ecuacion implicita queda
//! una CUARTICA, y hay que resolverla de verdad. Es la razon de que este
//! aca y no una esfera achatada que se le pareciera de lejos.
//!
//! LA ECUACION. Un toro de radio mayor `R` (del centro al centro del tubo)
//! y radio menor `r` (el grosor del tubo), con el eje en z:
//!
//!     (x^2 + y^2 + z^2 + R^2 - r^2)^2 = 4 R^2 (x^2 + y^2)
//!
//! Se lee bien: el lado izquierdo dice "que tan lejos estoy del centro" y
//! el derecho corrige por el agujero. Sustituyendo P = O + tD con D
//! unitaria y agrupando en potencias de t sale
//!
//!     t^4 + c3 t^3 + c2 t^2 + c1 t + c0 = 0
//!
//! y de ahi la resuelve `cuartica`, con el metodo de Ferrari.
//!
//! EN f64 Y NO EN f32, que es la decision que hace que el anillo se vea y
//! no titile. Los coeficientes llevan `p^2` y `(R^2 - r^2)^2` adentro: con
//! R = 1.4 y r = 0.12 eso es elevar a la cuarta, y los siete digitos de un
//! f32 no alcanzan cuando dos terminos grandes se restan y el resultado es
//! chico, que es exactamente lo que pasa en los rayos RASANTES, los que
//! entran casi tangentes al tubo. En f32 esos rayos se pierden o se
//! duplican segun el cuadro, y en pantalla el anillo hierve. Las cuentas
//! van en f64 y solo el resultado vuelve a f32.

use crate::material::Material;
use crate::ray_intersect::{Intersect, RayIntersect};
use crate::vec3::{cross, dot, normalize, Vec3};
use std::f32::consts::PI;

/// El epsilon con el que se descarta un impacto pegado al origen del rayo.
/// Es el mismo criterio que usan las demas primitivas: sin el, un rayo que
/// sale de la superficie se vuelve a chocar con ella.
const EPS: f64 = 1e-4;

pub struct Toro {
    /// El centro del agujero.
    pub centro: Vec3,
    /// El eje del anillo: la normal al plano en el que vive. Se normaliza
    /// al construirlo.
    eje: Vec3,
    /// Los dos ejes del plano del anillo, que salen del eje. Se guardan
    /// armados porque hacen falta en CADA rayo y armarlos son dos
    /// productos cruz.
    u: Vec3,
    v: Vec3,
    /// Del centro del agujero al centro del tubo.
    pub radio_mayor: f32,
    /// El grosor del tubo.
    pub radio_menor: f32,
    pub material: Material,
}

impl Toro {
    pub fn nuevo(
        centro: Vec3,
        eje: Vec3,
        radio_mayor: f32,
        radio_menor: f32,
        material: Material,
    ) -> Toro {
        let (eje, u, v) = Self::base(eje);
        Toro { centro, eje, u, v, radio_mayor, radio_menor, material }
    }

    /// Una base ortonormal con `eje` de tercer vector.
    ///
    /// El vector auxiliar se elige mirando cual componente del eje es la
    /// mas chica: cruzar con uno casi paralelo da un resultado diminuto y
    /// normalizarlo amplifica el error hasta que la base deja de ser
    /// ortogonal y el anillo sale torcido.
    fn base(eje: Vec3) -> (Vec3, Vec3, Vec3) {
        let w = normalize(&eje);
        let auxiliar = if w.x.abs() < w.y.abs() && w.x.abs() < w.z.abs() {
            Vec3::new(1.0, 0.0, 0.0)
        } else if w.y.abs() < w.z.abs() {
            Vec3::new(0.0, 1.0, 0.0)
        } else {
            Vec3::new(0.0, 0.0, 1.0)
        };
        let u = normalize(&cross(&auxiliar, &w));
        let v = cross(&w, &u);
        (w, u, v)
    }

    /// GIRA EL ANILLO. La animacion le cambia el eje en cada cuadro, y con
    /// el eje hay que rehacer la base.
    pub fn set_eje(&mut self, eje: Vec3) {
        let (w, u, v) = Self::base(eje);
        self.eje = w;
        self.u = u;
        self.v = v;
    }

    pub fn material_mut(&mut self) -> &mut Material {
        &mut self.material
    }

    /// El rayo en las coordenadas del anillo: `(origen, direccion)` con el
    /// eje del toro en z y el centro en el origen.
    fn al_local(&self, origin: &Vec3, direction: &Vec3) -> ([f64; 3], [f64; 3]) {
        let o = origin - self.centro;
        (
            [
                dot(&o, &self.u) as f64,
                dot(&o, &self.v) as f64,
                dot(&o, &self.eje) as f64,
            ],
            [
                dot(direction, &self.u) as f64,
                dot(direction, &self.v) as f64,
                dot(direction, &self.eje) as f64,
            ],
        )
    }

    /// La distancia al primer impacto por delante del origen, si hay.
    fn primer_impacto(&self, origin: &Vec3, direction: &Vec3, hasta: f64) -> Option<f64> {
        // DESCARTE BARATO: la esfera que envuelve al anillo. La mayoria de
        // los rayos de un cuadro no le pegan a esta cosa, y una cuadratica
        // cuesta una decima parte de una cuartica.
        let oc = origin - self.centro;
        let radio = (self.radio_mayor + self.radio_menor) as f64;
        let b = dot(direction, &oc) as f64;
        let c = dot(&oc, &oc) as f64 - radio * radio;
        if c > 0.0 {
            let disc = b * b - c;
            if disc < 0.0 {
                return None;
            }
            // Y si la esfera entera queda detras o mas lejos que el limite,
            // tampoco hay nada que resolver.
            let entrada = -b - disc.sqrt();
            if entrada > hasta {
                return None;
            }
        }

        // SEGUNDO DESCARTE: la LOSA del anillo. El toro entero vive a menos
        // de `radio_menor` de su plano; si el rayo cruza esa losa fuera de la
        // esfera (o no la cruza), no le pega. Para un anillo acostado —como
        // los halos de invocacion, que son chatos y anchos— la esfera
        // envolvente es casi toda aire, y esta prueba ahorra la cuartica a la
        // mayoria de los rayos que pasan por encima o por debajo.
        let (entra_esfera, sale_esfera) = {
            let disc = (b * b - c).max(0.0).sqrt();
            (-b - disc, -b + disc)
        };
        let dn = dot(direction, &self.eje) as f64;
        let on = dot(&oc, &self.eje) as f64;
        let grosor = self.radio_menor as f64;
        if dn.abs() < 1e-9 {
            if on.abs() > grosor {
                return None;
            }
        } else {
            let (mut t0, mut t1) = ((-grosor - on) / dn, (grosor - on) / dn);
            if t0 > t1 {
                std::mem::swap(&mut t0, &mut t1);
            }
            if t1 < entra_esfera || t0 > sale_esfera || t1 < EPS || t0 > hasta {
                return None;
            }
        }

        let (o, d) = self.al_local(origin, direction);
        let (rr, r2) = ((self.radio_mayor as f64).powi(2), (self.radio_menor as f64).powi(2));

        // D es unitaria, asi que d.d = 1 y el coeficiente de t^4 es 1.
        let n = o[0] * d[0] + o[1] * d[1] + o[2] * d[2];
        let p = o[0] * o[0] + o[1] * o[1] + o[2] * o[2];
        let k = p + rr - r2;
        // Lo que aporta el agujero: solo las componentes del plano.
        let e2 = d[0] * d[0] + d[1] * d[1];
        let e1 = o[0] * d[0] + o[1] * d[1];
        let e0 = o[0] * o[0] + o[1] * o[1];

        let c3 = 4.0 * n;
        let c2 = 2.0 * k + 4.0 * n * n - 4.0 * rr * e2;
        let c1 = 4.0 * n * k - 8.0 * rr * e1;
        let c0 = k * k - 4.0 * rr * e0;

        let mut mejor = f64::INFINITY;
        for t in cuartica(c3, c2, c1, c0).into_iter().flatten() {
            if t > EPS && t < mejor && t < hasta {
                mejor = t;
            }
        }
        mejor.is_finite().then_some(mejor)
    }

    /// La normal en un punto de la superficie.
    ///
    /// Sale del gradiente de la ecuacion implicita, que para el toro tiene
    /// una lectura geometrica directa: del punto al CENTRO DEL TUBO que le
    /// toca. Ese centro es el punto del circulo de radio `R` que esta en su
    /// misma direccion, o sea el punto proyectado al plano y reescalado a
    /// `R`.
    fn normal_en(&self, punto: &Vec3) -> Vec3 {
        let rel = punto - self.centro;
        let x = dot(&rel, &self.u);
        let y = dot(&rel, &self.v);
        let largo = (x * x + y * y).sqrt();
        if largo < 1e-6 {
            return self.eje;
        }
        let centro_tubo =
            self.centro + self.u * (x / largo * self.radio_mayor) + self.v * (y / largo * self.radio_mayor);
        normalize(&(punto - &centro_tubo))
    }
}

impl RayIntersect for Toro {
    fn ray_intersect<'a>(&'a self, origin: &Vec3, direction: &Vec3) -> Intersect<'a> {
        let Some(t) = self.primer_impacto(origin, direction, f64::INFINITY) else {
            return Intersect::empty();
        };

        let distancia = t as f32;
        let punto = origin + direction * distancia;
        let normal = self.normal_en(&punto);

        // UV: la vuelta GRANDE en u y la del tubo en v, que es como se
        // envuelve un anillo.
        let rel = &punto - self.centro;
        let x = dot(&rel, &self.u);
        let y = dot(&rel, &self.v);
        let z = dot(&rel, &self.eje);
        let radial = (x * x + y * y).sqrt() - self.radio_mayor;
        let u = 0.5 + y.atan2(x) / (2.0 * PI);
        let v = 0.5 + z.atan2(radial) / (2.0 * PI);

        Intersect::new(punto, normal, distancia, &self.material, u, v)
    }

    fn occluded(&self, origin: &Vec3, direction: &Vec3, max_distance: f32) -> bool {
        if self.material.emission_color.is_some() {
            return false;
        }
        self.primer_impacto(origin, direction, max_distance as f64).is_some()
    }

    fn puede_tapar(&self) -> bool {
        self.material.emission_color.is_none()
    }

    fn bounds(&self) -> Option<(Vec3, f32)> {
        Some((self.centro, self.radio_mayor + self.radio_menor))
    }

    /// LA CAJA EXACTA, que para un anillo inclinado es bastante mas chica
    /// que el cubo de la esfera acotante.
    ///
    /// Sobre cada eje del mundo, el anillo llega hasta
    /// `R * sqrt(1 - w_i^2) + r`, donde `w_i` es la componente del eje del
    /// toro en ese eje del mundo: un anillo acostado no mide nada de alto,
    /// y la esfera acotante pretende que mide `R + r`.
    fn aabb(&self) -> Option<(Vec3, Vec3)> {
        let extremo = |w: f32| {
            self.radio_mayor * (1.0 - w * w).max(0.0).sqrt() + self.radio_menor
        };
        let e = Vec3::new(extremo(self.eje.x), extremo(self.eje.y), extremo(self.eje.z));
        Some((self.centro - e, self.centro + e))
    }
}

/// LAS RAICES REALES DE `t^4 + c3 t^3 + c2 t^2 + c1 t + c0`, por Ferrari.
///
/// Devuelve hasta cuatro, en cualquier orden, con `None` en las que no
/// existen. No se ordenan: quien llama se queda con la menor que le sirva,
/// y ordenar cuatro numeros para elegir uno es trabajo de mas en el camino
/// mas caliente del trazador.
///
/// EL METODO, en tres pasos:
///   1. DEPRIMIR: con `t = y - c3/4` se va el termino cubico y queda
///      `y^4 + p y^2 + q y + r`, que tiene tres coeficientes en vez de
///      cuatro.
///   2. FACTORIZAR EN DOS CUADRATICAS. Se busca `s` tal que el depreso sea
///      `(y^2 + s y + u)(y^2 - s y + v)`. Igualando coeficientes sale que
///      `s^2` tiene que ser raiz de la CUBICA RESOLVENTE
///      `m^3 + 2p m^2 + (p^2 - 4r) m - q^2`, y de ahi salen `u` y `v`.
///   3. Resolver las dos cuadraticas, que ya es de primer anio.
///
/// El caso `q ~ 0` se atiende aparte: ahi el depreso es BICUADRATICO
/// (`y^4 + p y^2 + r`) y se resuelve como una cuadratica en `y^2`. No es un
/// lujo: le pasa a todo rayo que cruza el plano del anillo por su centro,
/// que en esta escena es una fila entera de pixeles.
fn cuartica(c3: f64, c2: f64, c1: f64, c0: f64) -> [Option<f64>; 4] {
    let corrimiento = c3 / 4.0;
    let p = c2 - 6.0 * corrimiento * corrimiento;
    let q = c1 - 2.0 * c2 * corrimiento + 8.0 * corrimiento * corrimiento * corrimiento;
    let r = c0 - c1 * corrimiento + c2 * corrimiento * corrimiento
        - 3.0 * corrimiento * corrimiento * corrimiento * corrimiento;

    let mut raices = [None; 4];
    let mut n = 0;
    let mut anotar = |y: f64| {
        if n < 4 {
            // De vuelta a la variable original, y una pasada de Newton
            // sobre la cuartica ORIGINAL para limpiar lo que se perdio en
            // el camino de ida y vuelta. Una sola: con dos no cambia nada
            // medible y el toro esta en el camino caliente.
            let mut t = y - corrimiento;
            let f = ((t + c3) * t + c2) * t * t + c1 * t + c0;
            let df = ((4.0 * t + 3.0 * c3) * t + 2.0 * c2) * t + c1;
            if df.abs() > 1e-12 {
                t -= f / df;
            }
            raices[n] = Some(t);
            n += 1;
        }
    };

    if q.abs() < 1e-12 {
        // Bicuadratica: y^2 = (-p +/- sqrt(p^2 - 4r)) / 2.
        let disc = p * p - 4.0 * r;
        if disc < 0.0 {
            return raices;
        }
        let s = disc.sqrt();
        for cuadrado in [(-p + s) / 2.0, (-p - s) / 2.0] {
            if cuadrado >= 0.0 {
                let y = cuadrado.sqrt();
                anotar(y);
                anotar(-y);
            }
        }
        return raices;
    }

    // La cubica resolvente. Se toma la raiz mas grande: tiene que ser
    // positiva para poder sacarle la raiz cuadrada, y el producto de las
    // tres raices es q^2 > 0, asi que la mayor lo es.
    let m = mayor_raiz_cubica(2.0 * p, p * p - 4.0 * r, -q * q);
    if m <= 0.0 {
        return raices;
    }
    let s = m.sqrt();
    let u = (p + m - q / s) / 2.0;
    let v = (p + m + q / s) / 2.0;

    // (y^2 + s y + u) y (y^2 - s y + v)
    for (b, c) in [(s, u), (-s, v)] {
        let disc = b * b - 4.0 * c;
        if disc < 0.0 {
            continue;
        }
        let raiz = disc.sqrt();
        anotar((-b + raiz) / 2.0);
        anotar((-b - raiz) / 2.0);
    }

    raices
}

/// La raiz real mas grande de `m^3 + a2 m^2 + a1 m + a0`.
///
/// Cardano con el caso trigonometrico: cuando la cubica tiene tres raices
/// reales, la formula de Cardano pasa por raices de numeros negativos y en
/// aritmetica real no se puede seguir. La salida clasica es escribir las
/// tres con cosenos, que ademas es mas estable.
fn mayor_raiz_cubica(a2: f64, a1: f64, a0: f64) -> f64 {
    let corrimiento = a2 / 3.0;
    let p = a1 - a2 * a2 / 3.0;
    let q = 2.0 * a2 * a2 * a2 / 27.0 - a2 * a1 / 3.0 + a0;

    let disc = q * q / 4.0 + p * p * p / 27.0;
    if disc > 0.0 {
        // Una sola raiz real.
        let raiz = disc.sqrt();
        let a = (-q / 2.0 + raiz).cbrt();
        let b = (-q / 2.0 - raiz).cbrt();
        a + b - corrimiento
    } else {
        // Tres reales: las tres con cosenos, y se devuelve la mayor, que es
        // la del angulo mas chico.
        let radio = (-p / 3.0).max(0.0).sqrt();
        if radio < 1e-15 {
            return -corrimiento;
        }
        let coseno = (-q / 2.0) / (radio * radio * radio);
        let angulo = coseno.clamp(-1.0, 1.0).acos() / 3.0;
        2.0 * radio * angulo.cos() - corrimiento
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::texture::Texture;
    use raylib::prelude::Color;

    fn anillo(eje: Vec3) -> Toro {
        Toro::nuevo(
            Vec3::new(0.0, 0.0, 0.0),
            eje,
            2.0,
            0.5,
            Material::new([0.9, 0.1, 0.0, 0.0], 8.0, 0.0, Texture::Solid(Color::WHITE), None),
        )
    }

    /// De frente, el rayo pega en el borde de afuera del tubo: a `R + r`
    /// del centro, o sea a 7.5 de un origen puesto a 10.
    #[test]
    fn le_pega_al_borde_de_afuera() {
        let t = anillo(Vec3::new(0.0, 1.0, 0.0));
        let hit = t.ray_intersect(&Vec3::new(-10.0, 0.0, 0.0), &Vec3::new(1.0, 0.0, 0.0));

        assert!(hit.is_intersecting, "el rayo tendria que pegarle al anillo");
        assert!(
            (hit.distance - 7.5).abs() < 1e-3,
            "pego a {} y tendria que ser a 7.5",
            hit.distance
        );
        // Y la normal ahi apunta hacia afuera, o sea hacia el rayo.
        assert!(hit.normal.x < -0.99, "la normal es {:?}", hit.normal);
    }

    /// POR EL AGUJERO NO PEGA NADA. Es la prueba que separa a un toro de
    /// una rosquilla maciza, y la que se cae si la cuartica se resuelve
    /// mal: un solver que pierde raices suele perder justamente el par de
    /// adentro.
    #[test]
    fn por_el_agujero_se_pasa_de_largo() {
        let t = anillo(Vec3::new(0.0, 1.0, 0.0));
        let hit = t.ray_intersect(&Vec3::new(0.0, -10.0, 0.0), &Vec3::new(0.0, 1.0, 0.0));
        assert!(!hit.is_intersecting, "el rayo entro por el agujero y pego igual");

        // Y apenas corrido, ya en el tubo, si pega.
        let hit = t.ray_intersect(&Vec3::new(2.0, -10.0, 0.0), &Vec3::new(0.0, 1.0, 0.0));
        assert!(hit.is_intersecting, "el rayo pasa por el centro del tubo y no pego");
        assert!((hit.distance - 9.5).abs() < 1e-3, "pego a {}", hit.distance);
    }

    /// El anillo tiene CUATRO cruces sobre esa recta y hay que quedarse con
    /// el primero que este por delante. Poniendo el origen entre medio, el
    /// que corresponde es el de adentro.
    #[test]
    fn se_queda_con_el_primero_de_adelante() {
        let t = anillo(Vec3::new(0.0, 1.0, 0.0));
        // Desde el centro del agujero, mirando hacia afuera: el primero es
        // el borde interior del tubo, a R - r = 1.5.
        let hit = t.ray_intersect(&Vec3::new(0.0, 0.0, 0.0), &Vec3::new(1.0, 0.0, 0.0));
        assert!(hit.is_intersecting);
        assert!((hit.distance - 1.5).abs() < 1e-3, "pego a {}", hit.distance);
    }

    /// LA CUARTICA CONTRA LA FUERZA BRUTA.
    ///
    /// Es el test que de verdad prueba el solver. Para un puniado de rayos
    /// de direcciones distintas se busca el primer impacto de dos maneras:
    /// con Ferrari, y marchando por la recta en pasitos hasta que la
    /// ecuacion implicita cambia de signo. Las dos tienen que dar lo mismo.
    #[test]
    fn ferrari_da_lo_mismo_que_marchar_a_pasitos() {
        let t = anillo(normalize(&Vec3::new(0.3, 1.0, -0.2)));

        // La ecuacion implicita del toro en un punto del mundo.
        let implicita = |punto: Vec3| -> f64 {
            let rel = punto - t.centro;
            let (x, y, z) = (
                dot(&rel, &t.u) as f64,
                dot(&rel, &t.v) as f64,
                dot(&rel, &t.eje) as f64,
            );
            let (rr, r2) = ((t.radio_mayor as f64).powi(2), (t.radio_menor as f64).powi(2));
            let a = x * x + y * y + z * z + rr - r2;
            a * a - 4.0 * rr * (x * x + y * y)
        };

        let mut probados = 0;
        for i in 0..40 {
            let a = i as f32 * 0.7;
            let origen = Vec3::new(a.cos() * 6.0, (a * 1.3).sin() * 3.0, a.sin() * 6.0);
            let direccion = normalize(&(Vec3::new(0.0, 0.0, 0.0) - origen
                + Vec3::new((a * 2.1).sin(), (a * 0.9).cos(), (a * 1.7).sin()) * 1.4));

            let analitico = t.primer_impacto(&origen, &direccion, f64::INFINITY);

            // A pasitos: el primer cambio de signo, afinado por biseccion.
            const PASO: f64 = 0.002;
            let mut bruto = None;
            let mut anterior = implicita(origen);
            let mut d = PASO;
            while d < 14.0 {
                let actual = implicita(origen + direccion * d as f32);
                if anterior.signum() != actual.signum() {
                    let (mut lo, mut hi) = (d - PASO, d);
                    for _ in 0..40 {
                        let medio = (lo + hi) / 2.0;
                        if implicita(origen + direccion * medio as f32).signum() == anterior.signum() {
                            lo = medio;
                        } else {
                            hi = medio;
                        }
                    }
                    bruto = Some((lo + hi) / 2.0);
                    break;
                }
                anterior = actual;
                d += PASO;
            }

            match (analitico, bruto) {
                (Some(a), Some(b)) => {
                    assert!(
                        (a - b).abs() < 5e-3,
                        "rayo {i}: Ferrari dice {a:.4} y los pasitos dicen {b:.4}"
                    );
                    probados += 1;
                }
                (None, None) => {}
                (a, b) => panic!("rayo {i}: uno encontro impacto y el otro no: {a:?} contra {b:?}"),
            }
        }
        assert!(probados > 10, "solo {probados} rayos le pegaron: el test no esta midiendo nada");
    }

    /// La caja de un anillo ACOSTADO es chata, y ese es todo el motivo de
    /// escribirla a mano en vez de dejar la del cubo de la esfera.
    #[test]
    fn la_caja_de_un_anillo_acostado_es_chata() {
        let t = anillo(Vec3::new(0.0, 1.0, 0.0));
        let (min, max) = t.aabb().expect("el anillo tiene caja");

        assert!((max.y - 0.5).abs() < 1e-5, "de alto mide {}", max.y);
        assert!((max.x - 2.5).abs() < 1e-5, "de ancho mide {}", max.x);
        assert!((min.y + 0.5).abs() < 1e-5);
        // La esfera acotante, que es lo que habria sin esto, daria 2.5 de
        // alto: cinco veces mas.
        let (_, radio) = t.bounds().unwrap();
        assert!((radio - 2.5).abs() < 1e-5);
    }
}
