"""
Analiza un archivo de audio y exporta un JSON con datos de sincronizacion
para el raytracer del concierto de Tame Impala.

Uso:
  python3 analizar_audio.py loser.mp3
  python3 analizar_audio.py loser.mp3 --salida sync.json

Salida: un JSON con:
  - bpm: float
  - duracion: float (segundos)
  - fps_analisis: int (cuadros de analisis por segundo)
  - frames: array de objetos, uno por cuadro de analisis, con:
      - t: float (timestamp en segundos)
      - bass: float 0..1 (energia de graves: kick, bajo)
      - mid: float 0..1 (energia de medios: voces, sintetizadores)
      - high: float 0..1 (energia de agudos: hi-hat, platillos)
      - total: float 0..1 (energia total)
      - onset: bool (hay un golpe percusivo en este frame)
      - chroma: array de 12 floats 0..1 (energia de cada nota, do a si)
  - beats: array de floats (timestamps de cada beat)
  - secciones: array de {t: float, tipo: string, energia_media: float}

El raytracer lee este archivo al arrancar y lo indexa por tiempo.
Sin FFT en Rust, sin crates de DSP, sin dependencias nuevas.
"""

import json
import sys
import argparse
import numpy as np

try:
    import librosa
except ImportError:
    print("Necesitas librosa: pip install librosa soundfile --break-system-packages")
    sys.exit(1)


# Cuantos cuadros de analisis por segundo. 30 es mas que suficiente
# para sincronizar visualmente: el ojo no distingue cambios de luz
# mas rapidos que ~24 Hz, y 30 da margen.
FPS_ANALISIS = 30

# Frecuencias de corte para las bandas.
# Graves: 20-250 Hz (kick, bajo, toms)
# Medios: 250-4000 Hz (voces, sintetizadores, guitarra)
# Agudos: 4000-16000 Hz (hi-hat, platillos, presencia)
CORTE_BAJO = 250
CORTE_ALTO = 4000


def normalizar_0_1(arr):
    """Normaliza al rango 0..1. Si el rango es cero, devuelve ceros."""
    mn, mx = arr.min(), arr.max()
    if mx - mn < 1e-12:
        return np.zeros_like(arr)
    return (arr - mn) / (mx - mn)


def suavizar(arr, ventana=3):
    """Promedio movil para suavizar ruido frame a frame."""
    kernel = np.ones(ventana) / ventana
    return np.convolve(arr, kernel, mode="same")


def analizar(ruta_audio, fps=FPS_ANALISIS):
    """Analiza el audio y devuelve el diccionario de sincronizacion."""
    print(f"Cargando {ruta_audio}...")
    y, sr = librosa.load(ruta_audio, sr=22050, mono=True)
    duracion = librosa.get_duration(y=y, sr=sr)
    print(f"  Duracion: {duracion:.1f}s  Sample rate: {sr}")

    # ─── BPM y beats ─────────────────────────────────────────────
    print("Detectando tempo y beats...")
    tempo, beat_frames = librosa.beat.beat_track(y=y, sr=sr)
    # librosa puede devolver un array de un solo elemento para tempo
    bpm = float(np.atleast_1d(tempo)[0])
    beat_times = librosa.frames_to_time(beat_frames, sr=sr).tolist()
    print(f"  BPM: {bpm:.1f}  Beats detectados: {len(beat_times)}")

    # ─── Espectrograma ────────────────────────────────────────────
    # hop_length controla la resolucion temporal. Lo calculamos para
    # que cada columna del espectrograma corresponda a un cuadro de
    # analisis (1/fps segundos).
    hop = int(sr / fps)
    n_fft = 2048

    print("Calculando espectrograma...")
    S = np.abs(librosa.stft(y, n_fft=n_fft, hop_length=hop))
    freqs = librosa.fft_frequencies(sr=sr, n_fft=n_fft)
    n_frames = S.shape[1]

    # ─── Energia por bandas ───────────────────────────────────────
    print("Extrayendo energia por bandas...")
    mask_bass = freqs < CORTE_BAJO
    mask_mid = (freqs >= CORTE_BAJO) & (freqs < CORTE_ALTO)
    mask_high = freqs >= CORTE_ALTO

    # RMS por banda en cada frame del espectrograma
    bass_raw = np.sqrt(np.mean(S[mask_bass, :] ** 2, axis=0))
    mid_raw = np.sqrt(np.mean(S[mask_mid, :] ** 2, axis=0))
    high_raw = np.sqrt(np.mean(S[mask_high, :] ** 2, axis=0))
    total_raw = np.sqrt(np.mean(S ** 2, axis=0))

    # Suavizar y normalizar
    bass = normalizar_0_1(suavizar(bass_raw, 3))
    mid = normalizar_0_1(suavizar(mid_raw, 3))
    high = normalizar_0_1(suavizar(high_raw, 3))
    total = normalizar_0_1(suavizar(total_raw, 3))

    # ─── Deteccion de onsets (golpes percusivos) ──────────────────
    print("Detectando onsets...")
    onset_env = librosa.onset.onset_strength(y=y, sr=sr, hop_length=hop)
    # UMBRAL RELATIVO AL NIVEL LOCAL, no absoluto.
    #
    # Antes esto era `media_movil + 0.25`, y ese 0.25 es una constante
    # sumada sobre una envolvente normalizada GLOBALMENTE. El efecto es el
    # contrario del que se busca: en un tramo tranquilo la media movil vale
    # 0.10, el umbral queda en 0.35 y cualquier ataque lo cruza; en un
    # tramo denso la media movil ya vale 0.55, el umbral se va a 0.80 y no
    # lo cruza NADA. O sea que el detector se vuelve mas sordo cuanto mas
    # fuerte toca el tema.
    #
    # Se midio sobre el Great Fairy's Fountain Theme y es exactamente lo
    # que pasaba: 145 onsets repartidos como 29, 23, 39 y 22 por cada
    # veinte segundos en la primera mitad, y CERO entre el segundo 120 y el
    # 140, que es el climax. La escena se quedaba sin su unica fuente de
    # golpes justo en el momento mas grande de la cancion.
    #
    # Multiplicar en vez de sumar arregla eso: un pico dispara si sobresale
    # un 45% de su vecindario, valga ese vecindario 0.1 o 0.6. El sumando
    # chico que queda es solo un piso para no disparar sobre el silencio,
    # donde la media movil es casi cero y cualquier ruido la supera por
    # mucho en terminos relativos.
    #
    # Y el pico tiene que ser un maximo local DE VERDAD (mayor que el
    # cuadro anterior Y que el siguiente). Comparando solo con el anterior,
    # una subida sostenida dispara en cada cuadro de la subida y un solo
    # ataque deja cuatro o cinco onsets pegados.
    onset_env_norm = normalizar_0_1(onset_env)
    media_movil = suavizar(onset_env_norm, fps // 2)  # ventana de ~0.5s
    umbral = media_movil * 1.45 + 0.04
    # Asegurar misma longitud
    min_len = min(len(onset_env_norm), len(umbral), n_frames)
    onsets = np.zeros(n_frames, dtype=bool)
    for i in range(1, min_len - 1):
        if (onset_env_norm[i] > umbral[i] and
                onset_env_norm[i] > onset_env_norm[i - 1] and
                onset_env_norm[i] >= onset_env_norm[i + 1]):
            onsets[i] = True

    n_onsets = int(onsets.sum())
    print(f"  Onsets detectados: {n_onsets}")

    # ─── Chroma: QUE NOTA esta sonando ────────────────────────────
    #
    # Las tres bandas de arriba dicen CUANTA energia hay y donde, pero no
    # dicen nada de la melodia: un arpegio de arpa y un acorde sostenido
    # con la misma energia en medios son, para ellas, lo mismo. El chroma
    # si: reparte el espectro en las doce notas de la escala sin importar
    # la octava, asi que un do de cualquier altura cae siempre en el mismo
    # casillero.
    #
    # Para este tema es LO que hay que mirar: la Great Fairy's Fountain es
    # un arpegio, o sea que la pieza ES la melodia. Con esto cada hada de
    # la escena puede quedarse con una nota y encenderse cuando suena.
    #
    # Se usa la version CQT y no la de la FFT porque el CQT tiene los
    # filtros espaciados como las notas (logaritmicos), que es
    # exactamente lo que hace falta para separar semitonos en los graves.
    print("Extrayendo chroma (notas)...")
    chroma_notas = librosa.feature.chroma_cqt(y=y, sr=sr, hop_length=hop)

    # Se normaliza por el maximo de TODO el tema y no cuadro por cuadro:
    # normalizando cada cuadro, el silencio entre frases daria la misma
    # nota a tope que el climax, y las hadas no se apagarian nunca.
    tope = float(chroma_notas.max())
    if tope > 1e-9:
        chroma_notas = chroma_notas / tope

    # ─── Secciones (segmentacion estructural) ─────────────────────
    print("Segmentando secciones...")
    # Usamos la auto-similitud del espectrograma para encontrar
    # cambios de seccion. librosa.segment.agglomerative agrupa
    # frames similares.
    try:
        # Chromas dan mejor segmentacion que el espectrograma crudo
        # porque capturan armonia, no timbre
        bounds = librosa.segment.agglomerative(chroma_notas, k=10)
        bound_times = librosa.frames_to_time(bounds, sr=sr, hop_length=hop)

        secciones = []
        for idx, bt in enumerate(bound_times):
            # Calcular energia media de la seccion para clasificarla
            if idx < len(bound_times) - 1:
                f_start = bounds[idx]
                f_end = bounds[idx + 1]
            else:
                f_start = bounds[idx]
                f_end = n_frames
            energia_seccion = float(total[f_start:f_end].mean())

            # Clasificacion burda por energia
            if energia_seccion < 0.25:
                tipo = "intro_outro"
            elif energia_seccion < 0.45:
                tipo = "verso"
            elif energia_seccion < 0.65:
                tipo = "pre_coro"
            else:
                tipo = "coro"

            secciones.append({
                "t": round(float(bt), 3),
                "tipo": tipo,
                "energia_media": round(energia_seccion, 3),
            })
        print(f"  Secciones detectadas: {len(secciones)}")
    except Exception as e:
        print(f"  Segmentacion fallo ({e}), usando seccion unica")
        secciones = [{"t": 0.0, "tipo": "verso", "energia_media": 0.5}]

    # ─── Construir array de frames ────────────────────────────────
    print("Armando frames de salida...")
    frames = []
    for i in range(n_frames):
        t = i / fps
        frames.append({
            "t": round(t, 4),
            "bass": round(float(bass[i]), 4),
            "mid": round(float(mid[i]), 4),
            "high": round(float(high[i]), 4),
            "total": round(float(total[i]), 4),
            "onset": bool(onsets[i]),
            # Las doce notas, de do a si. Con tres decimales alcanza: son
            # un valor de 0 a 1 que termina moviendo el brillo de una
            # esferita, y cada decimal de mas son 66 KB de archivo.
            "chroma": [
                round(float(chroma_notas[n, i]), 3) if i < chroma_notas.shape[1] else 0.0
                for n in range(12)
            ],
        })

    resultado = {
        "bpm": round(bpm, 2),
        "duracion": round(duracion, 3),
        "fps_analisis": fps,
        "n_frames": n_frames,
        "beats": [round(b, 4) for b in beat_times],
        "secciones": secciones,
        "frames": frames,
    }

    return resultado


def imprimir_resumen(data):
    """Imprime un resumen legible del analisis."""
    print(f"\n{'='*60}")
    print(f"RESUMEN DEL ANALISIS")
    print(f"{'='*60}")
    print(f"BPM:        {data['bpm']}")
    print(f"Duracion:   {data['duracion']:.1f}s")
    print(f"Beats:      {len(data['beats'])}")
    print(f"Frames:     {data['n_frames']} ({data['fps_analisis']} fps)")

    print(f"\nSECCIONES:")
    for i, s in enumerate(data["secciones"]):
        fin = data["secciones"][i + 1]["t"] if i + 1 < len(data["secciones"]) else data["duracion"]
        dur = fin - s["t"]
        barra = "#" * int(s["energia_media"] * 40)
        print(f"  {s['t']:6.1f}s - {fin:6.1f}s  ({dur:5.1f}s)  "
              f"{s['tipo']:<12s}  E={s['energia_media']:.2f}  {barra}")

    # Distribucion de energia
    frames = data["frames"]
    bass_arr = np.array([f["bass"] for f in frames])
    total_arr = np.array([f["total"] for f in frames])
    n_onsets = sum(1 for f in frames if f["onset"])
    print(f"\nENERGIA:")
    print(f"  Bass  promedio: {bass_arr.mean():.3f}  max: {bass_arr.max():.3f}")
    print(f"  Total promedio: {total_arr.mean():.3f}  max: {total_arr.max():.3f}")
    print(f"  Onsets: {n_onsets}")

    if "chroma" in frames[0]:
        notas = np.array([f["chroma"] for f in frames])
        nombres = ["do", "do#", "re", "re#", "mi", "fa",
                   "fa#", "sol", "sol#", "la", "la#", "si"]
        medias = notas.mean(axis=0)
        orden = np.argsort(medias)[::-1]
        print("  Notas mas presentes: " + ", ".join(
            f"{nombres[n]} ({medias[n]:.2f})" for n in orden[:5]))

    # Tamano estimado del JSON
    import json
    tamano = len(json.dumps(data))
    print(f"\nTamano del JSON: {tamano / 1024:.0f} KB")


def main():
    parser = argparse.ArgumentParser(description="Analiza audio para el raytracer")
    parser.add_argument("audio", help="Ruta al archivo de audio (mp3, wav, ogg, flac)")
    parser.add_argument("--salida", "-o", default=None,
                        help="Ruta del JSON de salida (default: mismo nombre .json)")
    parser.add_argument("--fps", type=int, default=FPS_ANALISIS,
                        help=f"Frames de analisis por segundo (default: {FPS_ANALISIS})")
    args = parser.parse_args()

    if args.salida is None:
        from pathlib import Path
        args.salida = str(Path(args.audio).with_suffix(".json"))

    data = analizar(args.audio, fps=args.fps)
    imprimir_resumen(data)

    with open(args.salida, "w") as f:
        json.dump(data, f)
    print(f"\nGuardado: {args.salida}")


if __name__ == "__main__":
    main()
