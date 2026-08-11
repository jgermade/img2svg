/* tslint:disable */
/* eslint-disable */

/** Ajuste del contorno. Común a las dos segmentaciones. */
export interface FitOptions {
    /** Ajustador. Por omisión `"pixel"`, que dibuja la escalera tal cual. */
    fit?: "pixel" | "polygon" | "spline";
    /**
     * Desvío máximo, en píxeles, que pueden meter `"polygon"` y `"spline"`.
     * Ningún punto del contorno acaba más lejos que esto de lo que se dibuja.
     * Por omisión, 0.75 con `"polygon"` y 1.5 con `"spline"`.
     */
    fitTolerance?: number;
}



/** Opciones de `convertPhoto`. Todas opcionales. */
export interface PhotoOptions extends FitOptions {
    /** Bits por canal que se conservan al cuantizar, de 1 a 8. */
    colorPrecision?: number;
    /** Distancia de color por debajo de la cual dos píxeles son el mismo. */
    tolerance?: number;
    /** Pasadas que regularizan la paleta mirando el vecindario; 0 la apaga. */
    smoothing?: number;
    /** Colocar los vértices donde la imagen dice que está el borde, no en la retícula. */
    subpixel?: boolean;
    /** Alfa por debajo del cual un píxel se considera transparente, 0-255. */
    alphaThreshold?: number;
    /** Área mínima, en píxeles, para que una región sobreviva. */
    filterSpeckle?: number;
    /** Grosor mínimo, en píxeles, para que una región sobreviva. */
    minThickness?: number;
    /** Escalón de un degradado: separación mínima entre entradas de la paleta. */
    gradientStep?: number;
    /** Parte de la imagen que un color tiene que valer para tener entrada propia. */
    minColorShare?: number;
    /** Tope de colores de la paleta. */
    maxColors?: number;
    /** Quitar el color de fondo. */
    removeBackground?: boolean;
    /** Fondo impuesto, en hexadecimal, en vez del detectado. */
    background?: string;
    /**
     * Aviso de avance, de 0 a 1. Se llama como mucho una vez por cada tanto por
     * ciento.
     *
     * Sólo lo tiene el camino de foto: es el que puede tardar medio segundo en
     * una imagen de 4 Mpx. El de pixel art reduce la imagen a la rejilla en el
     * primer paso y a partir de ahí trabaja sobre unas decenas de píxeles de
     * lado, así que no hay avance que contar.
     */
    onProgress?: (fraction: number) => void;
}



/** Opciones de `convertRgba`. Todas opcionales. */
export interface PixelOptions extends FitOptions {
    /** Lado de la celda, en píxeles reales, en vez del detectado. */
    scale?: number;
    /** Origen de la rejilla. Sólo se usa si vienen los dos. */
    offsetX?: number;
    /** Origen de la rejilla. Sólo se usa si vienen los dos. */
    offsetY?: number;
    /** Distancia de color por debajo de la cual dos píxeles son el mismo. */
    tolerance?: number;
    /** Alfa por debajo del cual un píxel se considera transparente, 0-255. */
    alphaThreshold?: number;
    /** Lado del píxel del dibujo en el SVG de salida. */
    pixelSize?: number;
    /** Un `<path>` por color en vez de uno por región conexa. */
    mergeColors?: boolean;
    /** Quitar el damero de transparencia. */
    removeCheckerboard?: boolean;
    /** Quitar el color de fondo. */
    removeBackground?: boolean;
    /** Fondo impuesto, en hexadecimal, en vez del detectado. */
    background?: string;
}



/**
 * Resultado de la conversión, con los datos de la rejilla empleada.
 */
export class Conversion {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Color de fondo retirado, en hexadecimal, o `undefined`.
     */
    readonly background: string | undefined;
    /**
     * Tamaño de celda detectado en el eje Y, en píxeles reales.
     */
    readonly cellHeight: number;
    /**
     * Tamaño de celda detectado en el eje X, en píxeles reales.
     */
    readonly cellWidth: number;
    /**
     * Lado de la casilla del damero de transparencia encontrado, o `undefined`.
     */
    readonly checkerCell: number | undefined;
    /**
     * Fracción de imagen devuelta a transparente al quitar el damero.
     */
    readonly checkerCoverage: number | undefined;
    /**
     * Colores distintos del SVG, uno por `<path>`.
     */
    readonly colors: number;
    /**
     * Alto de la rejilla, en píxeles del dibujo.
     */
    readonly gridHeight: number;
    /**
     * Ancho de la rejilla, en píxeles del dibujo.
     */
    readonly gridWidth: number;
    readonly offsetX: number;
    readonly offsetY: number;
    /**
     * Elementos `<path>` del documento.
     */
    readonly paths: number;
    /**
     * Subtrazados emitidos, sumando todos los paths.
     */
    readonly subpaths: number;
    readonly svg: string;
}

/**
 * Resultado de una conversión de foto.
 *
 * Es un tipo aparte y no unos cuantos `undefined` más en [`Conversion`]: los
 * dos caminos no comparten casi ninguna cifra —no hay rejilla, ni celda, ni
 * damero, y sí un recuento de regiones— y así el `.d.ts` **gana** un tipo en
 * vez de que el que ya consume la página se llene de campos opcionales.
 */
export class PhotoConversion {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Color de fondo retirado, en hexadecimal, o `undefined`.
     */
    readonly background: string | undefined;
    readonly canvasHeight: number;
    /**
     * Ancho del lienzo, que es el del `viewBox`. No tiene por qué ser el de la
     * imagen: quitar el fondo recorta.
     */
    readonly canvasWidth: number;
    /**
     * Entradas de la paleta.
     */
    readonly colors: number;
    readonly paths: number;
    /**
     * Regiones conexas emitidas. Es la cifra que se mueve al tocar el filtrado
     * de motas, y la que dice si el SVG se puede abrir en un editor.
     */
    readonly regions: number;
    readonly subpaths: number;
    readonly svg: string;
}

/**
 * Convierte un búfer RGBA por el camino de foto.
 *
 * Va aparte de [`convert_rgba`] en vez de mirar una clave `mode` dentro de las
 * opciones porque son dos juegos de ajustes que no se solapan: una función por
 * segmentación deja que cada una lea sólo lo suyo, y que el `.d.ts` diga cuál
 * devuelve qué.
 */
export function convertPhoto(width: number, height: number, data: Uint8Array, options: PhotoOptions): PhotoConversion;

/**
 * Convierte un búfer RGBA (el que devuelve `ctx.getImageData()`) en SVG.
 */
export function convertRgba(width: number, height: number, data: Uint8Array, options: PixelOptions): Conversion;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_conversion_free: (a: number, b: number) => void;
    readonly conversion_background: (a: number) => [number, number];
    readonly conversion_cellHeight: (a: number) => number;
    readonly conversion_cellWidth: (a: number) => number;
    readonly conversion_checkerCell: (a: number) => [number, number];
    readonly conversion_checkerCoverage: (a: number) => [number, number];
    readonly conversion_colors: (a: number) => number;
    readonly conversion_gridHeight: (a: number) => number;
    readonly conversion_gridWidth: (a: number) => number;
    readonly conversion_offsetX: (a: number) => number;
    readonly conversion_offsetY: (a: number) => number;
    readonly conversion_paths: (a: number) => number;
    readonly conversion_subpaths: (a: number) => number;
    readonly conversion_svg: (a: number) => [number, number];
    readonly convertPhoto: (a: number, b: number, c: number, d: number, e: any) => [number, number, number];
    readonly convertRgba: (a: number, b: number, c: number, d: number, e: any) => [number, number, number];
    readonly photoconversion_regions: (a: number) => number;
    readonly photoconversion_canvasHeight: (a: number) => number;
    readonly photoconversion_canvasWidth: (a: number) => number;
    readonly photoconversion_colors: (a: number) => number;
    readonly photoconversion_paths: (a: number) => number;
    readonly photoconversion_subpaths: (a: number) => number;
    readonly __wbg_photoconversion_free: (a: number, b: number) => void;
    readonly photoconversion_background: (a: number) => [number, number];
    readonly photoconversion_svg: (a: number) => [number, number];
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
