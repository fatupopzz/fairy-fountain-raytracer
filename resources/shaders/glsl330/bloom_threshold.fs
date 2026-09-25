#version 330

// PASADA 1 del bloom: quedarse solo con lo que quema.
//
// La escena es casi negra a proposito, asi que lo unico que pasa el umbral
// son las esferas del anillo, los laseres y el orbe. Todo lo demas sale en
// cero y no aporta nada al desenfoque de las pasadas siguientes.

in vec2 fragTexCoord;
in vec4 fragColor;

// La textura que se esta dibujando. raylib la ata sola, no hay que buscar
// su location ni pasarsela.
uniform sampler2D texture0;

// Luminancia a partir de la cual un pixel alimenta el glow.
uniform float threshold;

out vec4 finalColor;

void main() {
    vec4 color = texture(texture0, fragTexCoord);

    // Luminancia perceptual, no el promedio de los canales: el verde pesa
    // siete veces mas que el azul para el ojo, y en esta escena (rosa,
    // magenta, cian) el promedio plano haria florecer los azules oscuros.
    float luma = dot(color.rgb, vec3(0.2126, 0.7152, 0.0722));

    // Umbral BLANDO. Con un corte duro (`if luma > threshold`), un pixel que
    // oscila alrededor del umbral entra y sale del bloom entre cuadro y
    // cuadro y el halo parpadea. Escalando por cuanto se pasa del umbral, el
    // que apenas lo cruza aporta casi nada y la transicion es continua.
    float contrib = max(0.0, luma - threshold) / max(luma, 0.001);

    finalColor = vec4(color.rgb * contrib, 1.0);
}
