//! LAS MARIPOSAS del anillo de pasto, como las del Bosque Kokiri.
//!
//! Cada una es un cuerpo y dos alas (cajas orientadas finitas) que aletean
//! rapido, y vuela por su cuenta alrededor de la plaza, sobre las flores de
//! `isla::flora`: un camino que da la vuelta a la isla despacio, sube y baja,
//! y se desvia un poco, con la cabeza siempre hacia donde va. Todo es funcion
//! del segundo, como el resto de la escena. Las alas tienen un brillo propio
//! chiquito para que de noche se sigan viendo, como luciernagas.

use crate::caja_orientada::{ejes_de, llevar, CajaOrientada};
use crate::grupo_acotado::GrupoAcotado;
use crate::material::Material;
use crate::ray_intersect::RayIntersect;
use crate::sync::SceneParams;
use crate::texture::Texture;
use crate::vec3::Vec3;
use raylib::prelude::Color;
use std::any::Any;
use std::f32::consts::PI;

/// Los colores de las alas: amarillo, blanco, celeste y rosa.
const COLORES: [(u8, u8, u8); 4] = [(255, 220, 70), (250, 250, 240), (120, 200, 255), (255, 140, 200)];
/// Cuantas mariposas hay.
const CUANTAS: usize = 8;

pub struct Mariposas {
    grupos: Vec<usize>,
}

pub fn armar(objetos: &mut Vec<Box<dyn RayIntersect + Send + Sync>>) -> Mariposas {
    let cuerpo = Material::new([1.0, 0.1, 0.0, 0.0], 10.0, 0.0, Texture::Solid(Color::new(50, 35, 30, 255)), None);
    let grupos = (0..CUANTAS)
        .map(|k| {
            let (r, g, b) = COLORES[k % COLORES.len()];
            let ala = Material::new(
                [1.0, 0.2, 0.0, 0.0],
                20.0,
                0.0,
                Texture::Solid(Color::new(r, g, b, 255)),
                Some(Color::new(r / 3, g / 3, b / 3, 255)),
            );
            let partes: Vec<Box<dyn RayIntersect + Send + Sync>> = vec![
                Box::new(CajaOrientada::nueva(Vec3::zeros(), Vec3::new(0.02, 0.02, 0.12), ejes_de(0.0, 0.0, 0.0), cuerpo.clone())),
                Box::new(CajaOrientada::nueva(Vec3::zeros(), Vec3::new(0.17, 0.006, 0.14), ejes_de(0.0, 0.0, 0.0), ala.clone())),
                Box::new(CajaOrientada::nueva(Vec3::zeros(), Vec3::new(0.17, 0.006, 0.14), ejes_de(0.0, 0.0, 0.0), ala)),
            ];
            objetos.push(Box::new(GrupoAcotado::new(partes)));
            objetos.len() - 1
        })
        .collect();
    Mariposas { grupos }
}

/// Donde esta la mariposa `k` en el segundo `t`.
fn camino(k: usize, t: f32) -> Vec3 {
    let kf = k as f32;
    // Da la vuelta a la isla, cada una a su velocidad y en su sentido.
    let sentido = if k % 2 == 0 { 1.0 } else { -1.0 };
    let angulo = kf * 2.0 * PI / CUANTAS as f32 + sentido * t * (0.10 + 0.03 * (kf % 3.0));
    // Entre el borde de la plaza (6) y el de la isla (7.4), yendo y viniendo.
    let radio = 6.7 + 0.45 * (t * (0.35 + 0.05 * kf) + kf).sin();
    // El cuadrado de la plaza pide un "radio" en forma de cuadrado: se
    // estira el circulo hacia las esquinas.
    let (c, s) = angulo.sin_cos();
    let cuadrado = 1.0 / c.abs().max(s.abs()).max(0.72);
    let altura = crate::isla::TECHO_ISLA + 0.45 + 0.25 * (t * (0.9 + 0.1 * kf) + kf * 2.0).sin() + 0.08 * (t * 3.1 + kf).sin();
    Vec3::new(s * radio * cuadrado.min(1.25), altura, c * radio * cuadrado.min(1.25))
}

impl Mariposas {
    pub fn actualizar(&self, objetos: &mut [Box<dyn RayIntersect + Send + Sync>], p: &SceneParams) {
        let t = p.tiempo;
        for (k, &g) in self.grupos.iter().enumerate() {
            let Some(grupo) = objetos.get_mut(g).and_then(|o| (o.as_mut() as &mut dyn Any).downcast_mut::<GrupoAcotado>()) else {
                continue;
            };
            let donde = camino(k, t);
            let adelante = camino(k, t + 0.1) - donde;
            let guinada = adelante.x.atan2(adelante.z);
            // Aletea rapido, y cada tanto planea con las alas abiertas.
            let planea = ((t * 0.7 + k as f32).sin() > 0.6) as i32 as f32;
            let aleteo = 0.15 + 0.95 * ((t * (22.0 + k as f32)).sin() * 0.5 + 0.5) * (1.0 - 0.8 * planea);
            let base = ejes_de(guinada, -0.15, 0.0);
            for (i, hijo) in grupo.children_mut().iter_mut().enumerate() {
                if let Some(c) = (hijo.as_mut() as &mut dyn Any).downcast_mut::<CajaOrientada>() {
                    if i == 0 {
                        c.colocar(donde, base);
                    } else {
                        let lado = if i == 1 { -1.0 } else { 1.0 };
                        let ejes = ejes_de(guinada, -0.15, lado * aleteo);
                        c.colocar(donde + llevar(&ejes, &Vec3::new(lado * 0.088, 0.0, 0.0)), ejes);
                    }
                }
            }
            grupo.recalcular_caja(0.0);
        }
    }
}
