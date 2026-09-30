//! NAVI y las NOTAS DE LA OCARINA.
//!
//! Navi es el hada de Link: una bola de luz celeste con cuatro alas que no
//! para de revolotear alrededor de su cabeza. Aca es una esfera emisiva, sus
//! cuatro alas (cubos finisimos y translucidos que aletean) y una LUZ
//! PUNTUAL que la sigue, asi que va dejando un brillo frio que se mueve
//! sobre la tunica, el escudo y el piso.
//!
//! Las notas son lo que sale de la ocarina: cada ataque del arpa suelta una
//! corchea que sube en espiral hacia la fuente y se apaga. Llevan los dos
//! colores de los botones de la ocarina en el juego: el azul del boton A y
//! el amarillo de los botones C.
//!
//! Como todo lo que se mueve en esta escena, la posicion de cada cosa es
//! funcion del segundo de la cancion, no un estado que avanza cuadro a
//! cuadro: el mismo segundo da siempre la misma imagen.

use crate::caja_orientada::{ejes_de, girar, llevar, CajaOrientada};
use crate::grupo_acotado::GrupoAcotado;
use crate::light::Light;
use crate::link::LinkVivo;
use crate::material::Material;
use crate::ray_intersect::RayIntersect;
use crate::sphere::Sphere;
use crate::sync::SceneParams;
use crate::texture::Texture;
use crate::vec3::{cross, normalize, Vec3};
use raylib::prelude::Color;
use std::any::Any;
use std::f32::consts::PI;

/// Radio del cuerpo de luz de Navi.
const NAVI_RADIO: f32 = 0.055;

/// Cuanto se aleja Navi del centro de su orbita, como mucho. Es el margen de
/// su caja en el arbol.
const NAVI_ALCANCE: f32 = 1.25;

/// Cuantas notas pueden estar en el aire a la vez.
const NOTAS: usize = 8;

/// Cuanto vive una nota, en segundos.
const NOTA_VIDA: f32 = 3.2;

pub struct NaviViva {
    grupo: usize,
    notas: usize,
    luz: usize,
}

fn emisivo(color: Color) -> Material {
    Material::new(
        [1.0, 0.0, 0.0, 0.0],
        1.0,
        0.0,
        Texture::Solid(Color::new(255, 255, 255, 255)),
        Some(color),
    )
}

/// Arma a Navi y las notas, y agrega la luz de Navi a la lista.
pub fn armar(
    objetos: &mut Vec<Box<dyn RayIntersect + Send + Sync>>,
    luces: &mut Vec<Light>,
    boca: Vec3,
) -> NaviViva {
    let centro = boca + Vec3::new(0.0, 0.3, 0.0);

    // El cuerpo y las alas. Las alas son cubos de medio milimetro de
    // espesor: vistas de canto casi desaparecen y de plano son una lamina
    // celeste, que es como se ven las alas de un hada.
    let mut piezas: Vec<Box<dyn RayIntersect + Send + Sync>> = vec![Box::new(Sphere {
        center: centro,
        radius: NAVI_RADIO,
        material: emisivo(Color::new(200, 240, 255, 255)),
    })];
    for _ in 0..4 {
        piezas.push(Box::new(CajaOrientada::nueva(
            centro,
            Vec3::new(0.12, 0.004, 0.06),
            ejes_de(0.0, 0.0, 0.0),
            emisivo(Color::new(70, 130, 190, 255)),
        )));
    }
    let grupo = objetos.len();
    objetos.push(Box::new(GrupoAcotado::con_margen(piezas, NAVI_ALCANCE)));

    // Las notas: cada una es un grupo de tres cajas (la cabeza, la plica y
    // el corchete). El grupo de todas tiene la caja de TODO el recorrido
    // posible, que es la que va al arbol; cada cuadro se achica sola a lo
    // que haya en el aire.
    let notas: Vec<Box<dyn RayIntersect + Send + Sync>> = (0..NOTAS)
        .map(|_| {
            let partes: Vec<Box<dyn RayIntersect + Send + Sync>> = (0..3)
                .map(|_| {
                    Box::new(CajaOrientada::nueva(
                        boca,
                        Vec3::zeros(),
                        ejes_de(0.0, 0.0, 0.0),
                        emisivo(Color::BLACK),
                    )) as Box<dyn RayIntersect + Send + Sync>
                })
                .collect();
            Box::new(GrupoAcotado::new(partes)) as Box<dyn RayIntersect + Send + Sync>
        })
        .collect();
    let (min, max) = recorrido_de_las_notas(boca);
    let indice_notas = objetos.len();
    objetos.push(Box::new(GrupoAcotado::con_caja(notas, min, max)));

    let luz = luces.len();
    luces.push(Light::new(centro, Color::new(140, 205, 255, 255), 1.1).con_alcance(0.9));

    NaviViva { grupo, notas: indice_notas, luz }
}

/// La caja que encierra todo lo que puede recorrer una nota.
fn recorrido_de_las_notas(boca: Vec3) -> (Vec3, Vec3) {
    let mut min = boca;
    let mut max = boca;
    for k in 0..=40 {
        for fase in 0..8 {
            let p = camino_de_nota(boca, k as f32 / 40.0, fase as f32 * 0.8);
            for e in 0..3 {
                min[e] = min[e].min(p[e]);
                max[e] = max[e].max(p[e]);
            }
        }
    }
    let m = Vec3::new(0.35, 0.35, 0.35);
    (min - m, max + m)
}

/// Donde esta una nota a la fraccion `s` de su vida. Sube y avanza hacia el
/// centro de la fuente, ondulando de costado.
fn camino_de_nota(boca: Vec3, s: f32, fase: f32) -> Vec3 {
    let hacia = normalize(&Vec3::new(-boca.x, 0.0, -boca.z));
    let costado = cross(&hacia, &Vec3::new(0.0, 1.0, 0.0));
    let sube = s * 2.4 + (s * PI).sin() * 0.25;
    let avanza = s * 1.9;
    let onda = (s * 7.0 + fase).sin() * 0.28 * s.sqrt();
    boca + hacia * (avanza + 0.12) + Vec3::new(0.0, sube, 0.0) + costado * onda
}

fn hash(n: u32, k: u32) -> f32 {
    let mut x = n.wrapping_mul(0x9e37_79b9) ^ k.wrapping_mul(0x85eb_ca6b);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2545_f491);
    x ^= x >> 13;
    ((x >> 16) & 0xFFFF) as f32 / 65535.0
}

fn suave(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

impl NaviViva {
    pub fn actualizar(
        &self,
        objetos: &mut [Box<dyn RayIntersect + Send + Sync>],
        luces: &mut [Light],
        link: &LinkVivo,
        p: &SceneParams,
    ) {
        let t = p.tiempo;

        // ---- NAVI ----
        // Una orbita que no es un circulo: el angulo avanza con un seno
        // encima (acelera y frena), el radio respira y la altura sube y baja
        // al doble de frecuencia. Es la trayectoria nerviosa de un hada, y no
        // se repite igual nunca porque los periodos no se dividen.
        // Cuando la camara esta encima de Link (los primeros planos, el del
        // corazon) Navi se abre y sube, y brilla menos: pasaba justo delante
        // de la cara y su luz la quemaba, y no se veia ni el parpadeo.
        let enfoque = crate::plano_en(t).3.clamp(0.0, 1.0);
        let centro = link.cabeza(p) + Vec3::new(0.0, 0.22 + 0.30 * enfoque, 0.0);
        let angulo = t * 1.6 + (t * 0.53).sin() * 1.4;
        let radio = 0.52 + 0.14 * (t * 1.07).sin() + 0.45 * enfoque;
        let alto = 0.12 * (t * 2.3).sin() + 0.06 * (t * 5.1).sin();
        let pos = |a: f32, r: f32, h: f32| centro + Vec3::new(a.cos() * r, h, a.sin() * r);
        let aqui = pos(angulo, radio, alto);
        let adelante = normalize(&(pos(angulo + 0.05, radio, alto) - aqui));

        // Late con el tiempo fuerte.
        let brillo = (0.75 + 0.45 * p.pulso + 0.25 * p.swell) * (1.0 - 0.5 * enfoque);
        let c = |x: f32| (x * brillo).clamp(0.0, 255.0) as u8;

        if let Some(g) = objetos
            .get_mut(self.grupo)
            .and_then(|o| (o.as_mut() as &mut dyn Any).downcast_mut::<GrupoAcotado>())
        {
            let hijos = g.children_mut();
            if let Some(cuerpo) = hijos
                .first_mut()
                .and_then(|h| (h.as_mut() as &mut dyn Any).downcast_mut::<Sphere>())
            {
                cuerpo.center = aqui;
                cuerpo.material.emission_color = Some(Color::new(c(190.0), c(235.0), c(255.0), 255));
            }

            // Las alas: dos arriba y dos abajo, a cada lado, abisagradas en
            // el cuerpo y aleteando a unos seis golpes por segundo.
            let arriba = Vec3::new(0.0, 1.0, 0.0);
            let derecha = normalize(&cross(&adelante, &arriba));
            let aleteo = (t * 38.0).sin() * 0.7;
            for (k, hijo) in hijos.iter_mut().skip(1).enumerate() {
                let Some(ala) = (hijo.as_mut() as &mut dyn Any).downcast_mut::<CajaOrientada>() else {
                    continue;
                };
                let lado = if k % 2 == 0 { 1.0 } else { -1.0 };
                let de_arriba = k < 2;
                // Hacia donde sale el ala: al costado, levantada (las de
                // arriba) o caida (las de abajo), y un poco hacia atras.
                let alzada = if de_arriba { 0.45 } else { -0.35 } + aleteo;
                let afuera = girar(&(derecha * lado), &adelante, -alzada * lado);
                let afuera = normalize(&(afuera - adelante * 0.35));
                let normal = normalize(&cross(&afuera, &adelante));
                let largo_eje = normalize(&cross(&afuera, &normal));
                let tam = if de_arriba { 0.13 } else { 0.09 };
                ala.colocar(aqui + afuera * (tam * 0.5 + NAVI_RADIO * 0.6), [afuera, normal, largo_eje]);
                ala.medio = Vec3::new(tam * 0.5, 0.002, tam * 0.28);
                ala.material.emission_color = Some(Color::new(c(60.0), c(120.0), c(185.0), 255));
            }
            g.recalcular_caja(0.02);
        }

        if let Some(luz) = luces.get_mut(self.luz) {
            luz.position = aqui;
            luz.intensity = (0.8 + 0.6 * p.pulso) * (1.0 - 0.6 * enfoque);
        }

        // ---- LAS NOTAS ----
        let boca = link.ocarina(p);
        let mut vivas: Vec<(usize, f32)> = p
            .ataques
            .iter()
            .filter_map(|&(n, t0)| {
                let edad = t - t0;
                (0.0..NOTA_VIDA).contains(&edad).then_some((n, edad))
            })
            .collect();
        // Las mas nuevas primero: si hay mas que lugares, se pierden las
        // que ya se estaban apagando.
        vivas.sort_by(|a, b| a.1.total_cmp(&b.1));
        vivas.truncate(NOTAS);

        let Some(g) = objetos
            .get_mut(self.notas)
            .and_then(|o| (o.as_mut() as &mut dyn Any).downcast_mut::<GrupoAcotado>())
        else {
            return;
        };
        for (ranura, hijo) in g.children_mut().iter_mut().enumerate() {
            let Some(nota) = (hijo.as_mut() as &mut dyn Any).downcast_mut::<GrupoAcotado>() else {
                continue;
            };
            let (n, edad) = vivas.get(ranura).copied().unwrap_or((0, -1.0));
            let s = (edad / NOTA_VIDA).clamp(0.0, 1.0);
            // Aparece de golpe (un quinto de segundo) y se apaga despacio.
            let escala = if edad < 0.0 { 0.0 } else { suave(edad / 0.2) * (1.0 - suave((s - 0.6) / 0.4)) };
            let centro = camino_de_nota(boca, s, hash(n as u32, 1) * 6.0);
            let giro = edad * 1.8 + hash(n as u32, 2) * PI * 2.0;
            let ejes = ejes_de(giro, 0.0, 0.18 * (edad * 3.0).sin());

            // A o C: azul o amarillo.
            let color = if hash(n as u32, 3) < 0.4 {
                (90.0, 150.0, 255.0)
            } else {
                (255.0, 215.0, 70.0)
            };
            let fuerza = 0.55 + 0.45 * escala;
            let emision = Color::new(
                (color.0 * fuerza) as u8,
                (color.1 * fuerza) as u8,
                (color.2 * fuerza) as u8,
                255,
            );

            // (centro, tamano, alabeo) de la cabeza, la plica y el corchete,
            // en el sistema de la nota. La nota mide unos veinte centimetros.
            const PARTES: [((f32, f32), (f32, f32, f32), f32); 3] = [
                ((0.0, 0.0), (0.085, 0.06, 0.035), 0.35),
                ((0.036, 0.095), (0.016, 0.19, 0.016), 0.0),
                ((0.062, 0.165), (0.06, 0.018, 0.016), -0.65),
            ];
            for (hijo, &((x, y), (sx, sy, sz), alabeo)) in nota.children_mut().iter_mut().zip(PARTES.iter()) {
                if let Some(caja) = (hijo.as_mut() as &mut dyn Any).downcast_mut::<CajaOrientada>() {
                    let k = escala * 1.15;
                    let local = Vec3::new(x, y - 0.09, 0.0) * k;
                    let ejes_parte = crate::caja_orientada::componer(&ejes, &ejes_de(0.0, 0.0, alabeo));
                    caja.colocar(centro + llevar(&ejes, &local), ejes_parte);
                    caja.medio = Vec3::new(sx, sy, sz) * (0.5 * k);
                    caja.material.emission_color = Some(emision);
                }
            }
            nota.recalcular_caja(0.0);
        }
        g.recalcular_caja(0.0);
    }
}
