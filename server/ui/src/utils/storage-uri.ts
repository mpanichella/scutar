/**
 * Build and parse storage URIs for Leviathan (rclone-style).
 * Used to drive structured forms per storage type.
 */

export type StorageType = 'local' | 's3' | 'gcs' | 'azure' | 'sftp' | 'custom'

export interface LocalFields {
  path: string
}

export interface S3Fields {
  bucket: string
  region: string
  prefix: string
}

export interface GCSFields {
  bucket: string
  prefix: string
}

export interface AzureFields {
  container: string
  prefix: string
}

export interface SFTPFields {
  host: string
  port: number
  user: string
  path: string
}

export interface CustomFields {
  uri: string
}

export type StorageFields =
  | { type: 'local'; fields: LocalFields }
  | { type: 's3'; fields: S3Fields }
  | { type: 'gcs'; fields: GCSFields }
  | { type: 'azure'; fields: AzureFields }
  | { type: 'sftp'; fields: SFTPFields }
  | { type: 'custom'; fields: CustomFields }

const DEFAULT_S3: S3Fields = { bucket: '', region: 'us-east-1', prefix: '' }
const DEFAULT_GCS: GCSFields = { bucket: '', prefix: '' }
const DEFAULT_AZURE: AzureFields = { container: '', prefix: '' }
const DEFAULT_SFTP: SFTPFields = { host: '', port: 22, user: '', path: '/' }
const DEFAULT_LOCAL: LocalFields = { path: '/data' }
const DEFAULT_CUSTOM: CustomFields = { uri: '' }

export function buildUri(type: StorageType, fields: LocalFields | S3Fields | GCSFields | AzureFields | SFTPFields | CustomFields): string {
  switch (type) {
    case 'local':
      return (fields as LocalFields).path || ''
    case 's3': {
      const s = fields as S3Fields
      if (!s.bucket) return ''
      const p = s.prefix?.replace(/^\/+/, '') || ''
      return p ? `s3:${s.bucket}/${p}` : `s3:${s.bucket}`
    }
    case 'gcs': {
      const g = fields as GCSFields
      if (!g.bucket) return ''
      const p = g.prefix?.replace(/^\/+/, '') || ''
      return p ? `gcs:${g.bucket}/${p}` : `gcs:${g.bucket}`
    }
    case 'azure': {
      const a = fields as AzureFields
      if (!a.container) return ''
      const p = a.prefix?.replace(/^\/+/, '') || ''
      return p ? `azure:${a.container}/${p}` : `azure:${a.container}`
    }
    case 'sftp': {
      const f = fields as SFTPFields
      const path = (f.path || '/').trim()
      const pathNorm = path.startsWith('/') ? path : `/${path}` || '/'
      return `sftp:${pathNorm}`
    }
    case 'custom':
      return (fields as CustomFields).uri || ''
    default:
      return ''
  }
}

export function parseUri(uri: string): { type: StorageType; fields: LocalFields | S3Fields | GCSFields | AzureFields | SFTPFields | CustomFields } {
  const u = (uri || '').trim()
  if (!u) {
    return { type: 'local', fields: { ...DEFAULT_LOCAL } }
  }
  const lower = u.toLowerCase()
  if (lower.startsWith('s3:') || lower.startsWith('s3://')) {
    const rest = u.replace(/^s3:?\/?\/?/i, '')
    const [bucket, ...prefixParts] = rest.split('/')
    const prefix = prefixParts.join('/')
    return {
      type: 's3',
      fields: { bucket: bucket || '', region: 'us-east-1', prefix: prefix || '' },
    }
  }
  if (lower.startsWith('gcs:')) {
    const rest = u.replace(/^gcs:/i, '')
    const [bucket, ...prefixParts] = rest.split('/')
    const prefix = prefixParts.join('/')
    return {
      type: 'gcs',
      fields: { bucket: bucket || '', prefix: prefix || '' },
    }
  }
  if (lower.startsWith('azure:')) {
    const rest = u.replace(/^azure:/i, '')
    const [container, ...prefixParts] = rest.split('/')
    const prefix = prefixParts.join('/')
    return {
      type: 'azure',
      fields: { container: container || '', prefix: prefix || '' },
    }
  }
  if (lower.startsWith('sftp:') || lower.includes('sftp://')) {
    let path = '/'
    if (u.startsWith('sftp://')) {
      try {
        const url = new URL(u)
        path = url.pathname || '/'
        return {
          type: 'sftp',
          fields: {
            host: url.hostname || '',
            port: url.port ? parseInt(url.port, 10) : 22,
            user: url.username ? decodeURIComponent(url.username) : '',
            path,
          },
        }
      } catch {
        path = u.replace(/^sftp:?\/?\/?/i, '').trim() || '/'
      }
    } else {
      path = u.replace(/^sftp:/i, '').trim() || '/'
    }
    if (!path.startsWith('/')) path = `/${path}`
    return { type: 'sftp', fields: { ...DEFAULT_SFTP, path } }
  }
  if (u.startsWith('/') || lower.startsWith('local:')) {
    const path = u.replace(/^local:/i, '').trim() || u
    return { type: 'local', fields: { path } }
  }
  return { type: 'custom', fields: { uri: u } }
}

export function getDefaultFields(type: StorageType): LocalFields | S3Fields | GCSFields | AzureFields | SFTPFields | CustomFields {
  switch (type) {
    case 'local':
      return { ...DEFAULT_LOCAL }
    case 's3':
      return { ...DEFAULT_S3 }
    case 'gcs':
      return { ...DEFAULT_GCS }
    case 'azure':
      return { ...DEFAULT_AZURE }
    case 'sftp':
      return { ...DEFAULT_SFTP }
    case 'custom':
      return { ...DEFAULT_CUSTOM }
    default:
      return { ...DEFAULT_CUSTOM }
  }
}
