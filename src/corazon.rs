//! EL CONTENEDOR DE CORAZON: el premio del final.
//!
//! En Ocarina of Time, cuando Link consigue algo, lo levanta con los dos
//! brazos sobre la cabeza mientras suena la fanfarria: la pose mas conocida
//! de toda la serie. Y el Contenedor de Corazon (un corazon rojo que gira,
//! brillando) es el premio mas clasico. Aca cierra la escena: despues del
//! Fuego de Din aparece sobre el cuenco, crece girando, baja flotando hasta
//! Link, y Link lo levanta sobre la cabeza (ver `link::pose_de`). Al final
//! se deshace en luz.
//!
//! Es un corazon de voxeles, como todo lo demas: veintisiete cubos
//! orientados (giran con el) con el dibujo del icono del juego, y dos capas
//! para que tenga cuerpo. Rojo brillante, con emision propia, y una luz
//! rosada que tine lo que tiene cerca.

use crate::caja_orientada::{ejes_de, llevar, CajaOrientada};
use crate::grupo_acotado::GrupoAcotado;
use crate::light::Light;
use crate::material::Material;
use crate::ray_intersect::RayIntersect;
use crate::sync::SceneParams;
use crate::texture::Texture;
use crate::vec3::Vec3;
use raylib::prelude::Color;
use std::any::Any;

/// Cuando aparece sobre el cuenco, cuando llega a Link (y Link lo levanta),
/// y cuando se deshace en luz. En segundos de la cancion: despues del Fuego
/// de Din, en la coda.
pub const APARECE: f32 = 156.5;
pub const LLEGA: f32 = 166.0;
pub const SE_VA: f32 = 176.5;

/// El dibujo del corazon, fila por fila de arriba a abajo.
const DIBUJO: [&str; 6] = [".XX.XX.", "XXXXXXX", "XXXXXXX", ".XXXXX.", "..XXX..", "...X..."];
/// El lado de cada voxel.
const LADO: f32 = 0.075;

pub struct Corazon {
    grupo: usize,
    luz: usize,
    /// Donde va cada voxel respecto del centro, sin girar.
    voxeles: Vec<Vec3>,
}

fn suave(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Donde flota, cuanto mide (0 a 1) y cuanto brilla, en el segundo `t`.
fn estado(t: f32) -> (Vec3, f32, f32) {
    let sobre_cuenco = Vec3::new(0.0, crate::fuente::CUENCO_Y + 2.3, 0.0);
    // Sobre las manos de Link, que las levanta hasta ahi.
    let sobre_link = crate::link::PIES + Vec3::new(0.0, 2.02, -0.10);
    let baja = suave((t - (APARECE + 2.5)) / (LLEGA - APARECE - 2.5));
    // Baja en un arco: primero hacia Link y despues hacia abajo.
    let arco = Vec3::new(0.0, 0.6 * (baja * std::f32::consts::PI).sin(), 0.0);
    let flota = Vec3::new(0.0, 0.05 * (t * 2.2).sin(), 0.0);
    let donde = sobre_cuenco + (sobre_link - sobre_cuenco) * baja + arco + flota;
    let tam = suave((t - APARECE) / 1.2) * (1.0 - suave((t - SE_VA) / 1.2));
    // Brilla mas cuando Link lo levanta, y destella al irse.
    let brillo = 0.6 + 0.4 * suave((t - LLEGA) / 0.5) + 0.8 * suave((t - SE_VA) / 0.4) * (1.0 - suave((t - SE_VA - 0.6) / 0.6));
    (donde, tam, brillo)
}

pub fn armar(objetos: &mut Vec<Box<dyn RayIntersect + Send + Sync>>, luces: &mut Vec<Light>) -> Corazon {
    let rojo = Material::new(
        [0.7, 0.9, 0.25, 0.0],
        120.0,
        0.0,
        Texture::Solid(Color::new(235, 20, 45, 255)),
        Some(Color::BLACK),
    );
    let mut voxeles = Vec::new();
    let (ancho, alto) = (DIBUJO[0].len() as f32, DIBUJO.len() as f32);
    for (fila, linea) in DIBUJO.iter().enumerate() {
        for (col, ch) in linea.chars().enumerate() {
            if ch == 'X' {
                let x = (col as f32 - (ancho - 1.0) / 2.0) * LADO;
                let y = ((alto - 1.0) / 2.0 - fila as f32) * LADO;
                voxeles.push(Vec3::new(x, y, 0.0));
            }
        }
    }
    let cajas: Vec<Box<dyn RayIntersect + Send + Sync>> = voxeles
        .iter()
        .map(|_| {
            Box::new(CajaOrientada::nueva(Vec3::zeros(), Vec3::zeros(), ejes_de(0.0, 0.0, 0.0), rojo.clone()))
                as Box<dyn RayIntersect + Send + Sync>
        })
        .collect();
    let grupo = objetos.len();
    objetos.push(Box::new(GrupoAcotado::new(cajas)));
    let luz = luces.len();
    luces.push(Light::new(Vec3::zeros(), Color::new(255, 110, 140, 255), 0.0).con_alcance(1.8));
    Corazon { grupo, luz, voxeles }
}

impl Corazon {
    pub fn actualizar(&self, objetos: &mut [Box<dyn RayIntersect + Send + Sync>], luces: &mut [Light], p: &SceneParams) {
        let t = p.tiempo;
        let (donde, tam, brillo) = estado(t);
        // Gira sobre si mismo, como el item del juego.
        let ejes = ejes_de(t * 2.4, 0.0, 0.0);
        if let Some(g) = objetos
            .get_mut(self.grupo)
            .and_then(|o| (o.as_mut() as &mut dyn Any).downcast_mut::<GrupoAcotado>())
        {
            let late = 1.0 + 0.06 * p.pulso;
            for (hijo, v) in g.children_mut().iter_mut().zip(self.voxeles.iter()) {
                if let Some(c) = (hijo.as_mut() as &mut dyn Any).downcast_mut::<CajaOrientada>() {
                    c.colocar(donde + llevar(&ejes, &(*v * (tam * late))), ejes);
                    // Cada voxel un poco mas grande que su lugar, para que no
                    // queden rendijas; y el corazon con dos capas de grosor.
                    c.medio = Vec3::new(LADO * 0.52, LADO * 0.52, LADO * 1.0) * tam * late;
                    let k = brillo * 0.45;
                    c.material.emission_color =
                        Some(Color::new((235.0 * k) as u8, (20.0 * k) as u8, (60.0 * k) as u8, 255));
                }
            }
            g.recalcular_caja(0.0);
        }
        if let Some(luz) = luces.get_mut(self.luz) {
            luz.position = donde + Vec3::new(0.0, 0.1, 0.35);
            luz.intensity = 1.6 * tam * brillo;
        }
    }
}
