export type UserRole = 'admin' | 'manager' | 'member' | 'agent';

export type FieldType =
  | 'signature'
  | 'initials'
  | 'text'
  | 'date'
  | 'datestamp'
  | 'timestamp'
  | 'checkbox'
  | 'radio'
  | 'select'
  | 'multiple'
  | 'cells'
  | 'number'
  | 'phone'
  | 'file'
  | 'image'
  | 'payment'
  | 'verification'
  | 'kba'
  | 'heading'
  | 'note';

export interface BrandingConfig {
  product_name: string;
  primary_color: string;
  accent_color: string;
  primary_hsl?: string;
  primary_foreground_hsl?: string;
  accent_hsl?: string;
  accent_foreground_hsl?: string;
  logo_url: string;
  favicon_url: string;
  has_custom_logo?: boolean;
  has_custom_favicon?: boolean;
  email_sender_name?: string;
  email_footer?: string;
  support_email?: string;
  support_url?: string;
  terms_url?: string;
  privacy_url?: string;
  upstream_name: string;
  upstream_url: string;
}

export interface User {
  id: string;
  uuid: string;
  account_id: string;
  email: string;
  first_name: string;
  last_name: string;
  full_name?: string;
  role: UserRole;
  otp_required_for_login: boolean;
  signature_url?: string;
  initials_url?: string;
  archived_at?: string | null;
  created_at?: string;
  updated_at?: string;
}

export interface Account {
  id: string;
  uuid: string;
  name: string;
  timezone: string;
  locale: string;
  logo_url?: string;
  favicon_url?: string;
  testing?: boolean;
  linked_testing_account_id?: string;
  created_at?: string;
}

export interface TemplateFolder {
  id: string;
  account_id?: string;
  parent_folder_id?: string | null;
  name: string;
  full_name?: string;
  templates_count?: number;
  subfolders_count?: number;
  archived_at?: string | null;
  created_at?: string;
}

export interface FieldArea {
  x: number; // normalized 0..1
  y: number; // normalized 0..1
  w: number; // normalized 0..1
  h: number; // normalized 0..1
  page: number; // 0-indexed page number
  attachment_uuid?: string;
  option_uuid?: string;
  cell_w?: number;
}

export interface FieldOption {
  uuid: string;
  value: string;
}

export interface FieldCondition {
  field_uuid: string;
  operator: 'equal' | 'not_equal' | 'not_empty' | 'empty' | 'contains';
  value?: string;
  action?: 'show' | 'hide' | 'require';
}

export interface TemplateField {
  uuid: string;
  submitter_uuid: string;
  name: string;
  type: FieldType;
  required: boolean;
  readonly?: boolean;
  default_value?: string;
  title?: string;
  description?: string;
  placeholder?: string;
  preferences?: {
    format?: string;
    align?: 'left' | 'center' | 'right';
    font_size?: number;
    color?: string;
    formula?: string;
    pattern?: string;
    mask?: boolean;
    price?: number;
    currency?: string;
    with_signature_id?: boolean;
  };
  options?: FieldOption[];
  conditions?: FieldCondition[];
  areas: FieldArea[];
}

export interface SubmitterRole {
  uuid: string;
  name: string;
  color_index?: number;
  optional?: boolean;
  order?: number;
}

export interface TemplateDocumentPage {
  number: number; // 1-indexed or 0-indexed
  width: number;
  height: number;
  preview_url: string;
}

export interface TemplateDocument {
  id?: string;
  uuid: string;
  name: string;
  url: string;
  preview_images?: string[];
  pages?: TemplateDocumentPage[];
  metadata?: {
    pdf?: {
      number_of_pages?: number;
    };
    width?: number;
    height?: number;
  };
}

export interface SchemaItem {
  attachment_uuid: string;
  name: string;
}

export interface Template {
  id: string;
  slug: string;
  name: string;
  folder_id?: string;
  folder_name?: string;
  author_id?: string;
  author_name?: string;
  source: 'native' | 'api' | 'embed';
  external_id?: string | null;
  shared_link: boolean;
  schema: SchemaItem[];
  fields: TemplateField[];
  submitters: SubmitterRole[];
  documents: TemplateDocument[];
  preferences?: Record<string, unknown>;
  html_body?: string;
  html_head?: string;
  is_dynamic_html?: boolean;
  submissions_count?: number;
  archived_at?: string | null;
  created_at: string;
  updated_at: string;
}

export type SubmitterStatus = 'awaiting' | 'sent' | 'opened' | 'completed' | 'declined';

export interface Submitter {
  id: string;
  uuid: string;
  submission_id: string;
  slug: string;
  role: string;
  submitter_uuid?: string;
  name: string;
  email: string;
  phone?: string;
  status: SubmitterStatus;
  values: Record<string, unknown>;
  preferences?: Record<string, unknown>;
  metadata?: Record<string, unknown>;
  ip?: string;
  ua?: string;
  timezone?: string;
  sent_at?: string | null;
  opened_at?: string | null;
  completed_at?: string | null;
  declined_at?: string | null;
  decline_reason?: string | null;
  embed_src?: string;
  documents?: Array<{ name: string; url: string }>;
  created_at?: string;
}

export interface SubmissionEvent {
  id: string;
  submission_id: string;
  submitter_id?: string | null;
  submitter_name?: string;
  submitter_email?: string;
  event_type: string;
  event_timestamp: string;
  data?: Record<string, unknown>;
}

export type SubmissionStatus = 'pending' | 'completed' | 'declined' | 'expired';

export interface Submission {
  id: string;
  slug: string;
  name: string;
  template_id: string;
  template_name?: string;
  source: string;
  status: SubmissionStatus;
  submitters_order: 'random' | 'preserved';
  submitters: Submitter[];
  events?: SubmissionEvent[];
  template_fields?: TemplateField[];
  template_submitters?: SubmitterRole[];
  documents?: TemplateDocument[];
  audit_trail_url?: string | null;
  combined_document_url?: string | null;
  expire_at?: string | null;
  completed_at?: string | null;
  archived_at?: string | null;
  created_at: string;
  updated_at?: string;
}

export interface SigningSession {
  submitter: Submitter;
  submission: Submission;
  template: Template;
  fields: TemplateField[];
  all_values: Record<string, unknown>;
  requires_email_verification?: boolean;
  email_verified?: boolean;
  allow_to_decline?: boolean;
  allow_typed_signature?: boolean;
  with_signature_id?: boolean;
}

export interface PdfSignatureVerification {
  verified: boolean;
  integrity_status: 'Valid' | 'Tampered' | 'Unsigned';
  sha256: string;
  filename: string;
  byte_size: number;
  pkcs7_present: boolean;
  byte_range_valid: boolean;
  byte_range?: [number, number, number, number];
  certificate?: {
    subject: string;
    issuer: string;
    serial_number: string;
    not_before: string;
    not_after: string;
    fingerprint_sha256: string;
    valid_now: boolean;
    self_signed: boolean;
  };
  timestamp?: {
    present: boolean;
    server_url?: string;
    gen_time?: string;
    status: string;
  };
  matching_submission?: {
    submission_id: string;
    submission_slug: string;
    template_name: string;
    completed_at: string;
    submitters: Array<{
      name: string;
      email: string;
      role: string;
      completed_at?: string;
      ip?: string;
    }>;
  } | null;
}

export interface WebhookEndpoint {
  id: string;
  url: string;
  events: string[];
  secret: Record<string, string>;
  hmac_secret?: string;
  created_at: string;
}

export interface WebhookEventRecord {
  id: string;
  uuid: string;
  webhook_url_id: string;
  webhook_url?: string;
  event_type: string;
  record_type: string;
  record_id: string;
  status: 'pending' | 'success' | 'error';
  last_status_code?: number;
  last_response_body?: string;
  attempts_count?: number;
  created_at: string;
}

export interface McpTokenRecord {
  id: string;
  name: string;
  token_prefix: string;
  token?: string; // Only returned on creation
  created_at: string;
}
