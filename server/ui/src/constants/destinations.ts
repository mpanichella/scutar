/**
 * Tipos de origen/destino soportados por Leviathan (sin marcas de herramientas).
 * Origen y destino pueden ser local o remoto (blob, S3, GCS, SFTP, etc.).
 */
export interface StorageOption {
  id: string
  label: string
  description: string
  /** Ejemplo de URI (origen o destino). */
  example: string
  /** Descripción corta para uso como origen (ej. "Carpeta local en el pod"). */
  descriptionSource?: string
  /** Ejemplo para origen (si difiere, ej. /data para local). */
  exampleSource?: string
  custom?: boolean
}

export const STORAGE_OPTIONS: StorageOption[] = [
  {
    id: 'local',
    label: 'Local',
    description: 'Ruta en el sistema de archivos del pod',
    example: 'local:/data/backup',
    descriptionSource: 'Carpeta local en el pod (ej. /data, /var/backup)',
    exampleSource: '/data',
  },
  {
    id: 's3',
    label: 'Amazon S3',
    description: 'Bucket y path en Amazon S3',
    example: 's3:bucket-name/path/to/folder',
    descriptionSource: 'Bucket y path en S3 (origen remoto)',
  },
  {
    id: 'gcs',
    label: 'Google Cloud Storage',
    description: 'Bucket y path en GCS',
    example: 'gcs:bucket-name/path',
    descriptionSource: 'Bucket y path en GCS (origen remoto)',
  },
  {
    id: 'azure',
    label: 'Azure Blob Storage',
    description: 'Container y path en Azure Blob',
    example: 'azure:container-name/path',
    descriptionSource: 'Container y path en Azure Blob (origen remoto)',
  },
  {
    id: 'sftp',
    label: 'SFTP',
    description: 'Servidor SFTP y ruta remota',
    example: 'sftp://user@host/path',
    descriptionSource: 'Servidor SFTP y ruta remota (origen)',
  },
  {
    id: 'custom',
    label: 'URI personalizada',
    description: 'Escribí la URI completa',
    example: '',
    descriptionSource: 'URI completa del origen (local o remoto)',
    custom: true,
  },
]

/** @deprecated Use STORAGE_OPTIONS */
export const DESTINATION_OPTIONS = STORAGE_OPTIONS

/** Presets de cron para programación. __none__ = ejecutar una sola vez. */
export const CRON_PRESETS: { value: string; label: string }[] = [
  { value: '__none__', label: 'Una sola vez' },
  { value: '0 2 * * *', label: 'Diario a las 2:00' },
  { value: '0 3 * * *', label: 'Diario a las 3:00' },
  { value: '0 0 * * 0', label: 'Semanal (domingo 00:00)' },
  { value: '0 3 * * 0', label: 'Semanal (domingo 3:00)' },
  { value: '0 2 * * 1', label: 'Semanal (lunes 2:00)' },
  { value: '0 0 1 * *', label: 'Mensual (día 1 a las 00:00)' },
  { value: '', label: 'Personalizado (escribir abajo)' },
]
