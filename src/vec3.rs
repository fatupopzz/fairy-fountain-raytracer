//! El vector de tres componentes y las tres operaciones que el trazador
//! necesita de el.
//!
//! Esto reemplaza a `nalgebra-glm`, que es lo unico que el proyecto usaba de
//! esa dependencia. Se conto: de toda la libreria se usaban el tipo `Vec3`,
//! las funciones libres `dot`, `cross` y `normalize`, el metodo
//! `magnitude` y los operadores aritmeticos. Nada mas: ni matrices, ni
//! cuaterniones, ni transformaciones.
//!
//! POR QUE ESCRIBIRLO A MANO. Un trazador de rayos ES aritmetica de
//! vectores; delegarla en una libreria es delegar justamente la parte que
//! el proyecto se propone mostrar. Y `nalgebra` es una libreria de algebra
//! lineal generica de verdad —su `Vec3` es en realidad una `Matrix<f32, U3,
//! U1, ArrayStorage>`, con toda la maquinaria de tipos que eso arrastra—
//! para terminar haciendo tres multiplicaciones y dos sumas.
//!
//! La representacion es un struct llano de tres `f32` con `#[inline]` en
//! todo. No hay SIMD explicito a proposito: el compilador vectoriza estas
//! operaciones solo cuando le conviene, y escribirlo a mano con `f32x4`
//! obligaria a un cuarto componente de relleno que en este trazador no
//! paga (el cuello de botella son los recorridos del BVH y los fallos de
//! cache, no la aritmetica).

use std::ops::{Add, AddAssign, Div, Index, IndexMut, Mul, Neg, Sub};

/// Un punto o una direccion en el espacio.
///
/// `Copy` a proposito: son doce bytes y se pasan por valor millones de
/// veces por cuadro. Con `Clone` explicito el codigo se llenaria de
/// `.clone()` sin ganar nada.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    /// `const` para poder declarar vectores como constantes de modulo, que
    /// es como estan escritas las posiciones fijas de la escena.
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Vec3 { x, y, z }
    }

    /// El vector nulo.
    pub const fn zeros() -> Self {
        Vec3::new(0.0, 0.0, 0.0)
    }

    /// El largo del vector.
    #[inline]
    pub fn magnitude(self) -> f32 {
        self.magnitude_squared().sqrt()
    }

    /// El largo AL CUADRADO, que es el que conviene cuando solo hay que
    /// comparar dos distancias: se ahorra la raiz, y la raiz es de las
    /// pocas operaciones de punto flotante que todavia cuestan.
    #[inline]
    pub fn magnitude_squared(self) -> f32 {
        self.x * self.x + self.y * self.y + self.z * self.z
    }

    /// Producto punto, tambien como metodo: el codigo lo escribe de las
    /// dos formas, `dot(&a, &b)` y `a.dot(&b)`.
    #[inline]
    pub fn dot(self, o: &Vec3) -> f32 {
        dot(&self, o)
    }

    /// Alias de `magnitude`, que es como lo llamaba nalgebra y como esta
    /// escrito en algunos lugares del trazador.
    #[inline]
    pub fn norm(self) -> f32 {
        self.magnitude()
    }

    /// Producto componente a componente. No es ninguna operacion
    /// geometrica: es para escalar un color o un vector por otro, eje por
    /// eje.
    #[inline]
    pub fn component_mul(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x * o.x, self.y * o.y, self.z * o.z)
    }
}

/// Producto punto: cuanto se parecen dos direcciones.
///
/// Es la operacion mas usada del trazador con diferencia (el coseno entre
/// la normal y la luz, entre la normal y la vista, y cada prueba de
/// interseccion), asi que va marcada para que se inserte siempre.
#[inline]
pub fn dot(a: &Vec3, b: &Vec3) -> f32 {
    a.x * b.x + a.y * b.y + a.z * b.z
}

/// Producto cruz: el vector perpendicular a los dos, con la regla de la
/// mano derecha. De aca salen las normales de los triangulos y las bases
/// tangentes del mapeo de relieve.
#[inline]
pub fn cross(a: &Vec3, b: &Vec3) -> Vec3 {
    Vec3::new(
        a.y * b.z - a.z * b.y,
        a.z * b.x - a.x * b.z,
        a.x * b.y - a.y * b.x,
    )
}

/// El mismo vector con largo 1.
///
/// El vector nulo se devuelve tal cual en vez de dar `NaN`. Dividir por
/// cero aca no seria un error ruidoso sino un `NaN` que se propaga en
/// silencio por todo el sombreado y aparece como un pixel negro suelto
/// imposible de rastrear.
#[inline]
pub fn normalize(v: &Vec3) -> Vec3 {
    let n = v.magnitude();
    if n > 0.0 {
        Vec3::new(v.x / n, v.y / n, v.z / n)
    } else {
        *v
    }
}

// ---------- Operadores ----------
//
// Se implementan para `Vec3` y tambien para `&Vec3` en el lado izquierdo:
// el codigo del trazador escribe indistintamente `a + b` y `origin +
// direction * t` donde `origin` es una referencia, y sin estas variantes
// habria que sembrar asteriscos por todos lados.

macro_rules! binop {
    ($trait:ident, $metodo:ident, $op:tt) => {
        impl $trait<Vec3> for Vec3 {
            type Output = Vec3;
            #[inline]
            fn $metodo(self, o: Vec3) -> Vec3 {
                Vec3::new(self.x $op o.x, self.y $op o.y, self.z $op o.z)
            }
        }
        impl $trait<&Vec3> for Vec3 {
            type Output = Vec3;
            #[inline]
            fn $metodo(self, o: &Vec3) -> Vec3 {
                self $op *o
            }
        }
        impl $trait<Vec3> for &Vec3 {
            type Output = Vec3;
            #[inline]
            fn $metodo(self, o: Vec3) -> Vec3 {
                *self $op o
            }
        }
        impl $trait<&Vec3> for &Vec3 {
            type Output = Vec3;
            #[inline]
            fn $metodo(self, o: &Vec3) -> Vec3 {
                *self $op *o
            }
        }
    };
}

binop!(Add, add, +);
binop!(Sub, sub, -);
binop!(Mul, mul, *);
binop!(Div, div, /);

macro_rules! escalar {
    ($trait:ident, $metodo:ident, $op:tt) => {
        impl $trait<f32> for Vec3 {
            type Output = Vec3;
            #[inline]
            fn $metodo(self, k: f32) -> Vec3 {
                Vec3::new(self.x $op k, self.y $op k, self.z $op k)
            }
        }
        impl $trait<f32> for &Vec3 {
            type Output = Vec3;
            #[inline]
            fn $metodo(self, k: f32) -> Vec3 {
                *self $op k
            }
        }
    };
}

escalar!(Mul, mul, *);
escalar!(Div, div, /);

/// `k * v` ademas de `v * k`, que es como esta escrito en varios lugares.
impl Mul<Vec3> for f32 {
    type Output = Vec3;
    #[inline]
    fn mul(self, v: Vec3) -> Vec3 {
        v * self
    }
}

impl Neg for Vec3 {
    type Output = Vec3;
    #[inline]
    fn neg(self) -> Vec3 {
        Vec3::new(-self.x, -self.y, -self.z)
    }
}

impl Neg for &Vec3 {
    type Output = Vec3;
    #[inline]
    fn neg(self) -> Vec3 {
        -*self
    }
}

/// INDEXADO POR EJE: `v[0]`, `v[1]`, `v[2]`.
///
/// No es azucar sintactico, lo necesita el BVH. El test de rebanadas y la
/// division del arbol trabajan sobre UN eje elegido en tiempo de
/// ejecucion (el mas largo de la caja), y con solo `.x`, `.y`, `.z` habria
/// que escribir un `match` de tres ramas en cada uno de esos lugares.
impl Index<usize> for Vec3 {
    type Output = f32;
    #[inline]
    fn index(&self, i: usize) -> &f32 {
        match i {
            0 => &self.x,
            1 => &self.y,
            2 => &self.z,
            _ => panic!("un Vec3 no tiene eje {i}"),
        }
    }
}

impl IndexMut<usize> for Vec3 {
    #[inline]
    fn index_mut(&mut self, i: usize) -> &mut f32 {
        match i {
            0 => &mut self.x,
            1 => &mut self.y,
            2 => &mut self.z,
            _ => panic!("un Vec3 no tiene eje {i}"),
        }
    }
}

impl AddAssign<Vec3> for Vec3 {
    #[inline]
    fn add_assign(&mut self, o: Vec3) {
        self.x += o.x;
        self.y += o.y;
        self.z += o.z;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_producto_punto_mide_el_coseno() {
        let x = Vec3::new(1.0, 0.0, 0.0);
        let y = Vec3::new(0.0, 1.0, 0.0);
        assert_eq!(dot(&x, &x), 1.0);
        assert_eq!(dot(&x, &y), 0.0, "perpendiculares dan cero");
        assert_eq!(dot(&x, &-x), -1.0, "opuestos dan menos uno");
    }

    #[test]
    fn el_producto_cruz_sigue_la_mano_derecha() {
        let x = Vec3::new(1.0, 0.0, 0.0);
        let y = Vec3::new(0.0, 1.0, 0.0);
        assert_eq!(cross(&x, &y), Vec3::new(0.0, 0.0, 1.0));
        assert_eq!(cross(&y, &x), Vec3::new(0.0, 0.0, -1.0), "anticonmutativo");
    }

    #[test]
    fn normalizar_deja_largo_uno_y_no_rompe_con_el_cero() {
        let v = normalize(&Vec3::new(3.0, 4.0, 0.0));
        assert!((v.magnitude() - 1.0).abs() < 1e-6);
        assert_eq!(v, Vec3::new(0.6, 0.8, 0.0));

        // Lo importante: NO devuelve NaN. Un NaN aca se propagaria en
        // silencio por todo el sombreado.
        let cero = normalize(&Vec3::zeros());
        assert!(cero.x.is_finite() && cero.y.is_finite() && cero.z.is_finite());
    }

    #[test]
    fn la_aritmetica_va_componente_a_componente() {
        let a = Vec3::new(1.0, 2.0, 3.0);
        let b = Vec3::new(10.0, 20.0, 30.0);
        assert_eq!(a + b, Vec3::new(11.0, 22.0, 33.0));
        assert_eq!(b - a, Vec3::new(9.0, 18.0, 27.0));
        assert_eq!(a * 2.0, Vec3::new(2.0, 4.0, 6.0));
        assert_eq!(2.0 * a, a * 2.0, "el escalar conmuta");
        assert_eq!(&a + &b, a + b, "las referencias dan lo mismo");
        assert_eq!(a.component_mul(b), Vec3::new(10.0, 40.0, 90.0));
    }

    #[test]
    fn se_indexa_por_eje() {
        let mut v = Vec3::new(1.0, 2.0, 3.0);
        assert_eq!((v[0], v[1], v[2]), (1.0, 2.0, 3.0));
        v[1] = 9.0;
        assert_eq!(v.y, 9.0);
    }

    #[test]
    fn el_largo_al_cuadrado_evita_la_raiz() {
        let v = Vec3::new(3.0, 4.0, 12.0);
        assert_eq!(v.magnitude_squared(), 169.0);
        assert_eq!(v.magnitude(), 13.0);
    }
}
