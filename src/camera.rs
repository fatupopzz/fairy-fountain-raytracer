use nalgebra_glm::{cross, normalize, Vec3};

/// Camara libre: una posicion y dos angulos.
/// yaw gira a los lados, pitch mira arriba y abajo.
///
/// Es Copy porque el render se hace en otro hilo: se le manda una FOTO de
/// la camara del momento en que arranco. Si compartiera la de verdad, mover
/// la camara a mitad de un render le cambiaria el punto de vista por la
/// mitad de la imagen.
#[derive(Clone, Copy)]
pub struct Camera {
    pub position: Vec3,
    /// Rotacion horizontal, en radianes.
    pub yaw: f32,
    /// Rotacion vertical, en radianes.
    pub pitch: f32,
}

impl Camera {
    pub fn new(position: Vec3, yaw: f32, pitch: f32) -> Self {
        Camera {
            position,
            yaw,
            pitch,
        }
    }

    /// Hacia donde mira la camara. Con yaw = 0 y pitch = 0 da (0, 0, -1),
    /// que es la direccion original de la escena.
    pub fn get_forward(&self) -> Vec3 {
        Vec3::new(
            self.pitch.cos() * self.yaw.sin(),
            self.pitch.sin(),
            -self.pitch.cos() * self.yaw.cos(),
        )
    }

    /// El costado derecho de la camara. Sale de cruzar forward con el
    /// "arriba" del mundo, asi que siempre queda horizontal.
    pub fn get_right(&self) -> Vec3 {
        normalize(&cross(&self.get_forward(), &Vec3::new(0.0, 1.0, 0.0)))
    }

    /// Los tres ejes de la camara, listos para armar los rayos.
    pub fn basis(&self) -> (Vec3, Vec3, Vec3) {
        let forward = self.get_forward();
        let right = self.get_right();
        // El up real de la camara: perpendicular a los otros dos, asi
        // que se inclina junto con el pitch.
        let up = cross(&right, &forward);
        (right, up, forward)
    }
}
