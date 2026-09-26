//! Numeros al azar para el muestreo estocastico del trazador.
//!
//! De aca salen las direcciones que hacen que los reflejos sean BORROSOS y
//! las sombras SUAVES. La idea de fondo es una sola y se paga una sola vez:
//!
//!   un reflejo mate es el promedio de muchos reflejos perfectos en
//!   direcciones un poco distintas, y una sombra suave es el promedio de
//!   muchas sombras duras desde puntos un poco distintos de la luz.
//!
//! Hacer ese promedio de la forma obvia —tirar dieciseis rayos por pixel—
//! multiplicaria por dieciseis lo mas caro que hay en el cuadro. Pero la
//! escena YA tiene un acumulador temporal que promedia los ultimos cuatro
//! cuadros con reproyeccion (`Framebuffer::acumular`), asi que alcanza con
//! tirar UN rayo por pixel y moverlo un poco distinto en cada cuadro: el
//! acumulador hace el promedio gratis. El costo en rayos es exactamente
//! CERO, porque son los mismos rayos de siempre apuntados un poco distinto.
//!
//! Lo que eso pide es que el numero al azar cumpla dos cosas:
//!
//!   - que cambie entre CUADROS, o el promedio temporal promedia siempre la
//!     misma muestra y no converge a nada;
//!   - que cambie entre PIXELES VECINOS, o el ruido se organiza en bandas y
//!     se ve peor que el problema que vino a resolver.
//!
//! Por eso la semilla se arma con un hash de (x, y, cuadro) y no con un
//! generador con estado: un generador con estado daria numeros distintos
//! segun el ORDEN en que los hilos tomen los pixeles, y el trazado va en
//! paralelo con rayon. Con un hash, cada pixel calcula su propio azar sin
//! hablar con nadie y el cuadro sale igual se reparta como se reparta.

use crate::vec3::Vec3;

/// Mezclador de enteros de 32 bits.
///
/// Es un hash de avalancha de los de siempre (de la familia de los que usa
/// MurmurHash al final): multiplicar por una constante impar grande y
/// mezclar los bits de arriba con los de abajo. Lo unico que se le pide es
/// que cambiar UN bit de la entrada cambie la mitad de los bits de la
/// salida, que es justo lo que hace falta para que pixeles vecinos —cuyas
/// coordenadas difieren en un bit— no queden correlacionados.
#[inline]
pub fn revolver(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x
}

/// La semilla de un pixel en un cuadro.
#[inline]
pub fn semilla(x: usize, y: usize, cuadro: u32) -> u32 {
    revolver(x as u32)
        ^ revolver((y as u32).wrapping_mul(0x9e37_79b9))
        ^ revolver(cuadro.wrapping_mul(0x85eb_ca6b))
}

/// Un numero en [0, 1) a partir de la semilla.
///
/// Se queda con los 24 bits de arriba, que son los que el hash mezcla
/// mejor, y son ademas los que caben exactos en la mantisa de un `f32`.
#[inline]
pub fn uniforme(s: u32) -> f32 {
    (revolver(s) >> 8) as f32 / 16_777_216.0
}

/// Un punto al azar DENTRO de la esfera unitaria, repartido parejo.
///
/// Parejo de verdad, no "un vector de componentes al azar": tomando las
/// tres componentes de un cubo se acumulan puntos en las esquinas, y la
/// nube queda con forma de cubo redondeado en vez de esfera. Y la forma
/// habitual de arreglarlo —sortear en el cubo y descartar lo que cae
/// afuera— aca no sirve, porque descartar significa un bucle de largo
/// variable y esto se llama millones de veces por cuadro.
///
/// La receta sin bucle es: una direccion pareja sobre la esfera (la altura
/// `z` va uniforme, que es el resultado de Arquimedes: las fajas de igual
/// altura de una esfera tienen igual area) y un radio con RAIZ CUBICA del
/// uniforme, porque el volumen crece con el cubo del radio y sin la raiz
/// los puntos se amontonarian en el centro.
#[inline]
pub fn en_esfera(s: u32) -> Vec3 {
    let u1 = uniforme(s);
    let u2 = uniforme(s ^ 0x68bc_21eb);
    let u3 = uniforme(s ^ 0x02e5_be93);

    let z = 1.0 - 2.0 * u1;
    let r = (1.0 - z * z).max(0.0).sqrt();
    let phi = u2 * std::f32::consts::TAU;
    let radio = u3.cbrt();

    Vec3::new(r * phi.cos() * radio, r * phi.sin() * radio, z * radio)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_uniforme_se_queda_adentro_del_rango() {
        for i in 0..10_000u32 {
            let v = uniforme(i);
            assert!((0.0..1.0).contains(&v), "uniforme({i}) = {v}");
        }
    }

    #[test]
    fn el_uniforme_esta_bien_repartido() {
        // Diez cajones; con diez mil muestras cada uno tendria que llevarse
        // cerca de mil. Se acepta entre 850 y 1150: mas ajustado que eso
        // seria un test que falla por azar.
        let mut cajones = [0u32; 10];
        for i in 0..10_000u32 {
            cajones[(uniforme(i) * 10.0) as usize % 10] += 1;
        }
        for (i, &n) in cajones.iter().enumerate() {
            assert!((850..=1150).contains(&n), "cajon {i} se llevo {n}");
        }
    }

    #[test]
    fn pixeles_vecinos_no_quedan_correlacionados() {
        // Lo que de verdad importa: dos pixeles pegados tienen que dar
        // muestras distintas, o el ruido se organiza en bandas.
        let a = semilla(100, 100, 7);
        for (dx, dy) in [(1, 0), (0, 1), (1, 1)] {
            let b = semilla(100 + dx, 100 + dy, 7);
            assert_ne!(a, b, "vecino ({dx},{dy}) repite semilla");
            assert!(
                (uniforme(a) - uniforme(b)).abs() > 0.01,
                "vecino ({dx},{dy}) da casi la misma muestra"
            );
        }
        // Y el mismo pixel en el cuadro siguiente tambien, o el acumulador
        // temporal promedia siempre lo mismo y no converge.
        assert_ne!(semilla(100, 100, 7), semilla(100, 100, 8));
    }

    #[test]
    fn la_esfera_contiene_los_puntos_y_no_los_amontona() {
        let mut suma = Vec3::zeros();
        let mut dentro_del_medio = 0;
        const N: u32 = 20_000;
        for i in 0..N {
            let p = en_esfera(revolver(i));
            let r = p.magnitude();
            assert!(r <= 1.0001, "punto fuera de la esfera: {r}");
            if r < 0.5 {
                dentro_del_medio += 1;
            }
            suma += p;
        }
        // La media tiene que dar casi el origen: si la nube estuviera
        // corrida, los reflejos saldrian todos desviados hacia un lado.
        let media = suma / N as f32;
        assert!(
            media.magnitude() < 0.02,
            "la nube esta corrida: media {:?}, largo {}",
            media,
            media.magnitude()
        );
        // Y la media esfera interior tiene un octavo del volumen, asi que
        // le toca cerca del 12.5% de los puntos. Si la raiz cubica
        // faltara, este numero se iria por encima del 30%.
        let fraccion = dentro_del_medio as f32 / N as f32;
        assert!(
            (0.10..0.15).contains(&fraccion),
            "los puntos estan amontonados en el centro: {fraccion}"
        );
    }
}
