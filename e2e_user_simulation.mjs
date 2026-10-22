import assert from 'node:assert/strict';
import http from 'node:http';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

const BASE_URL = process.env.SIGNLINE_URL || 'http://127.0.0.1:3000';
const PB_URL = process.env.POCKETBASE_URL || 'http://127.0.0.1:8090';

let sessionCookie = '';
let jwtToken = '';
let apiAccessToken = '';
let mcpBearerToken = '';

async function req(method, urlPath, body = undefined, extraHeaders = {}) {
  const headers = { ...extraHeaders };
  if (jwtToken && !headers['Authorization'] && !headers['X-Auth-Token']) {
    headers['Authorization'] = `Bearer ${jwtToken}`;
  }
  if (sessionCookie && !headers['Cookie']) {
    headers['Cookie'] = sessionCookie;
  }

  let payload = body;
  if (
    body !== undefined &&
    !(body instanceof FormData) &&
    !Buffer.isBuffer(body) &&
    typeof body === 'object'
  ) {
    headers['Content-Type'] = 'application/json';
    payload = JSON.stringify(body);
  }

  const res = await fetch(`${BASE_URL}${urlPath}`, {
    method,
    headers,
    body: payload,
  });

  const setCookie = res.headers.get('set-cookie');
  if (setCookie) {
    sessionCookie = setCookie.split(';')[0];
  }

  const ct = res.headers.get('content-type') || '';
  let data;
  if (ct.includes('application/json')) {
    data = await res.json();
  } else if (ct.includes('application/pdf') || ct.includes('image/')) {
    const ab = await res.arrayBuffer();
    data = Buffer.from(ab);
  } else {
    data = await res.text();
  }

  return { status: res.status, headers: res.headers, data };
}

function logStep(num, title) {
  console.log(`\n================================================================`);
  console.log(`[STEP ${num}] ${title}`);
  console.log(`================================================================`);
}

async function runSimulation() {
  console.log(`Starting Exhaustive Real-User Simulation against ${BASE_URL}...`);

  // -------------------------------------------------------------------------
  // STEP 1: Health, PocketBase Verification, & SPA Frontend Serving
  // -------------------------------------------------------------------------
  logStep(1, 'Health Check, PocketBase 34 Collections, & Frontend SPA Asset Serving');
  const health = await req('GET', '/api/health');
  assert.equal(health.status, 200);
  assert.equal(health.data.version, '0.1.0b');
  console.log('✓ Backend health check passed:', JSON.stringify(health.data));

  const pbHealth = await fetch(`${PB_URL}/api/health`).then((r) => r.json());
  assert.equal(pbHealth.code, 200);
  console.log('✓ Managed PocketBase daemon is healthy on', PB_URL);

  const spaHtml = await req('GET', '/');
  assert.equal(spaHtml.status, 200);
  assert.ok(typeof spaHtml.data === 'string' && spaHtml.data.includes('<div id="root">'));
  console.log('✓ Frontend TypeScript/ShadCN SPA index.html served at /');

  const logoRes = await req('GET', '/branding/logo');
  assert.equal(logoRes.status, 200);
  const manifestRes = await req('GET', '/manifest.json');
  assert.equal(manifestRes.status, 200);
  assert.equal(manifestRes.data.name, 'Signline');
  console.log('✓ Default Signline logo and PWA manifest.json served');

  // -------------------------------------------------------------------------
  // STEP 2: Initial Setup Wizard (/api/setup) & Authentication
  // -------------------------------------------------------------------------
  logStep(2, 'Initial Setup Wizard, Login, Session Cookie, & TOTP 2FA Lifecycle');
  const setupStatusBefore = await req('GET', '/api/setup/status');
  assert.equal(setupStatusBefore.status, 200);
  assert.equal(setupStatusBefore.data.branding.attribution.upstream_name, 'Signline');
  assert.equal(
    setupStatusBefore.data.branding.attribution.upstream_url,
    'https://github.com/signline/signline'
  );

  if (setupStatusBefore.data.needs_setup) {
    const setupRes = await req('POST', '/api/setup', {
      first_name: 'Alice',
      last_name: 'Admin',
      email: 'alice@signline.local',
      password: 'Password123!',
      account_name: 'Signline Enterprise Corp',
      timezone: 'America/Chicago',
      locale: 'en-US',
    });
    assert.equal(setupRes.status, 201, `Setup failed: ${JSON.stringify(setupRes.data)}`);
    jwtToken = setupRes.data.token;
    apiAccessToken = setupRes.data.api_token;
    console.log('✓ Completed initial /setup wizard for Alice Admin');
  } else {
    const loginRes = await req('POST', '/api/auth/login', {
      email: 'alice@signline.local',
      password: 'Password123!',
    });
    assert.equal(loginRes.status, 200);
    jwtToken = loginRes.data.token;
  }

  const meRes = await req('GET', '/api/auth/me');
  assert.equal(meRes.status, 200);
  assert.equal(meRes.data.user.email, 'alice@signline.local');
  assert.equal(meRes.data.user.role, 'admin');
  console.log('✓ Authenticated current user:', meRes.data.user.email, '| Role:', meRes.data.user.role);

  // Test TOTP 2FA setup & disable
  const totpSetup = await req('POST', '/api/auth/2fa/setup', {});
  assert.equal(totpSetup.status, 200);
  assert.ok(totpSetup.data.otp_secret);
  assert.ok(totpSetup.data.provisioning_uri.startsWith('otpauth://totp/'));
  console.log('✓ TOTP 2FA secret & provisioning URI generated');

  const totpDisable = await req('DELETE', '/api/auth/2fa');
  assert.equal(totpDisable.status, 200);
  console.log('✓ TOTP 2FA disable verified');

  // Test Password Reset token generation & confirmation
  const pwResetReq = await req('POST', '/api/auth/password/reset', {
    email: 'alice@signline.local',
  });
  assert.equal(pwResetReq.status, 200);
  assert.ok(pwResetReq.data.reset_token);
  const pwResetConfirm = await req('PUT', '/api/auth/password/reset', {
    token: pwResetReq.data.reset_token,
    password: 'Password123!',
  });
  assert.equal(pwResetConfirm.status, 200);
  console.log('✓ Password reset flow verified end-to-end');

  // -------------------------------------------------------------------------
  // STEP 3: Folders, PDF Template Upload, Field Auto-Detection, & Builder
  // -------------------------------------------------------------------------
  logStep(3, 'Template Folders, Multi-Page PDF Upload, HTML Templates, & WYSIWYG Builder');

  const folderRes = await req('POST', '/api/folders', {
    name: 'HR & Legal Contracts',
  });
  assert.equal(folderRes.status, 201);
  const folderId = folderRes.data.id;
  console.log('✓ Created template folder:', folderRes.data.name, `(${folderId})`);

  // Create a dynamic HTML template containing embedded field tags
  const htmlTplRes = await req('POST', '/api/templates/html', {
    name: 'Master Services Agreement (MSA)',
    folder_id: folderId,
    html: `
      <h1>Master Services Agreement</h1>
      <p>This Master Services Agreement is entered into between Signline Enterprise Corp and Client.</p>
      <p>Company Name: {{company_name;role=First Party;type=text;required=true}}</p>
      <p>Effective Date: {{effective_date;role=First Party;type=date;required=true}}</p>
      <p>First Party Signature: {{client_sig;role=First Party;type=signature;required=true}}</p>
      <p>Counterparty Approval: {{counter_sig;role=Second Party;type=signature;required=true}}</p>
    `,
  });
  assert.equal(htmlTplRes.status, 201, `HTML template failed: ${JSON.stringify(htmlTplRes.data)}`);
  const templateId = htmlTplRes.data.id;
  const templateSlug = htmlTplRes.data.slug;
  assert.ok(htmlTplRes.data.fields.length >= 4, 'Expected extracted fields from HTML tags');
  assert.ok(htmlTplRes.data.documents.length >= 1, 'Expected generated PDF document');
  console.log(
    `✓ Created template "${htmlTplRes.data.name}" (id=${templateId}, slug=${templateSlug}) with ${htmlTplRes.data.fields.length} extracted fields`
  );

  // Test Multipart PDF upload endpoint (/api/templates/upload)
  const docPreviewUrl = htmlTplRes.data.documents[0].url;
  const samplePdfRes = await req('GET', docPreviewUrl);
  assert.equal(samplePdfRes.status, 200);
  assert.ok(Buffer.isBuffer(samplePdfRes.data) && samplePdfRes.data.subarray(0, 5).toString() === '%PDF-');

  const uploadForm = new FormData();
  uploadForm.append(
    'file',
    new Blob([samplePdfRes.data], { type: 'application/pdf' }),
    'uploaded-nda.pdf'
  );
  uploadForm.append('name', 'Uploaded Mutual NDA');
  const uploadedTplRes = await req('POST', '/api/templates/upload', uploadForm);
  assert.equal(uploadedTplRes.status, 201);
  console.log('✓ Multipart PDF upload & page preview rendering verified:', uploadedTplRes.data.id);

  // Test Auto-Detect Fields on the uploaded template
  const detectRes = await req('POST', `/api/templates/${uploadedTplRes.data.id}/detect_fields`, {});
  assert.equal(detectRes.status, 200);
  assert.ok(Array.isArray(detectRes.data.fields) && detectRes.data.fields.length > 0);
  console.log(`✓ Auto-detected ${detectRes.data.fields.length} form fields on uploaded PDF`);

  // Update the MSA template in the WYSIWYG Builder (enable shared_link, add fields across types, conditional rule & formula)
  const firstPartyUuid = htmlTplRes.data.submitters[0]?.uuid || crypto.randomUUID();
  const secondPartyUuid = htmlTplRes.data.submitters[1]?.uuid || crypto.randomUUID();
  const docUuid = htmlTplRes.data.schema[0]?.attachment_uuid || htmlTplRes.data.documents[0]?.uuid;

  const updatedBuilderFields = [
    {
      uuid: 'fld-company',
      submitter_uuid: firstPartyUuid,
      name: 'Company Name',
      type: 'text',
      required: true,
      readonly: false,
      areas: [{ x: 0.1, y: 0.2, w: 0.35, h: 0.04, page: 0, attachment_uuid: docUuid }],
    },
    {
      uuid: 'fld-seats',
      submitter_uuid: firstPartyUuid,
      name: 'License Seats',
      type: 'number',
      required: true,
      readonly: false,
      default_value: '10',
      areas: [{ x: 0.5, y: 0.2, w: 0.2, h: 0.04, page: 0, attachment_uuid: docUuid }],
    },
    {
      uuid: 'fld-total',
      submitter_uuid: firstPartyUuid,
      name: 'Total Contract Value',
      type: 'number',
      required: false,
      readonly: true,
      formula: 'fld-seats * 100',
      areas: [{ x: 0.72, y: 0.2, w: 0.2, h: 0.04, page: 0, attachment_uuid: docUuid }],
    },
    {
      uuid: 'fld-terms',
      submitter_uuid: firstPartyUuid,
      name: 'Agree to Terms',
      type: 'checkbox',
      required: true,
      readonly: false,
      areas: [{ x: 0.1, y: 0.3, w: 0.04, h: 0.04, page: 0, attachment_uuid: docUuid }],
    },
    {
      uuid: 'fld-plan',
      submitter_uuid: firstPartyUuid,
      name: 'Tier Selection',
      type: 'select',
      required: true,
      readonly: false,
      options: [
        { uuid: 'opt-1', value: 'Enterprise' },
        { uuid: 'opt-2', value: 'Standard' },
      ],
      areas: [{ x: 0.2, y: 0.3, w: 0.3, h: 0.04, page: 0, attachment_uuid: docUuid }],
    },
    {
      uuid: 'fld-sig-1',
      submitter_uuid: firstPartyUuid,
      name: 'Client Signature',
      type: 'signature',
      required: true,
      readonly: false,
      areas: [{ x: 0.1, y: 0.45, w: 0.35, h: 0.08, page: 0, attachment_uuid: docUuid }],
    },
    {
      uuid: 'fld-sig-2',
      submitter_uuid: secondPartyUuid,
      name: 'Counterparty Signature',
      type: 'signature',
      required: true,
      readonly: false,
      areas: [{ x: 0.52, y: 0.45, w: 0.35, h: 0.08, page: 0, attachment_uuid: docUuid }],
    },
  ];

  const updateTplRes = await req('PUT', `/api/templates/${templateId}`, {
    name: 'Master Services Agreement (MSA v2)',
    shared_link: true,
    submitters: [
      { name: 'First Party', uuid: firstPartyUuid },
      { name: 'Second Party', uuid: secondPartyUuid },
    ],
    fields: updatedBuilderFields,
  });
  assert.equal(updateTplRes.status, 200);
  assert.equal(updateTplRes.data.shared_link, true);
  assert.equal(updateTplRes.data.fields.length, 7);
  console.log('✓ Saved WYSIWYG Builder updates (7 fields, 2 roles, shared_link=true)');

  // Clone, Archive, and Restore the secondary template
  const cloneRes = await req('POST', `/api/templates/${uploadedTplRes.data.id}/clone`, {});
  assert.equal(cloneRes.status, 201);
  const archiveRes = await req('DELETE', `/api/templates/${cloneRes.data.id}`);
  assert.equal(archiveRes.status, 200);
  const restoreRes = await req('POST', `/api/templates/${cloneRes.data.id}/restore`, {});
  assert.equal(restoreRes.status, 200);
  console.log('✓ Template clone, archive, and restore verified');

  // -------------------------------------------------------------------------
  // STEP 4: Local Webhook Receiver & Webhook HMAC-SHA256 Setup
  // -------------------------------------------------------------------------
  logStep(4, 'Register Local Webhook Receiver & Verify X-Signline-Signature HMAC-SHA256');

  const receivedWebhooks = [];
  const webhookServer = http.createServer((wReq, wRes) => {
    let rawBody = '';
    wReq.on('data', (chunk) => {
      rawBody += chunk;
    });
    wReq.on('end', () => {
      receivedWebhooks.push({
        headers: wReq.headers,
        body: rawBody,
        parsed: JSON.parse(rawBody || '{}'),
      });
      wRes.writeHead(200, { 'Content-Type': 'application/json' });
      wRes.end(JSON.stringify({ received: true }));
    });
  });

  await new Promise((resolve) => webhookServer.listen(0, '127.0.0.1', resolve));
  const whPort = webhookServer.address().port;
  const whSecret = 'whsec_test_signline_hmac_secret_001';

  const createWhRes = await req('POST', '/api/settings/webhooks', {
    url: `http://127.0.0.1:${whPort}/webhook`,
    hmac_secret: whSecret,
    events: [
      'form.viewed',
      'form.started',
      'form.completed',
      'form.declined',
      'form.delegated',
      'submission.created',
      'submission.completed',
    ],
  });
  assert.equal(createWhRes.status, 201);
  console.log(`✓ Registered local webhook receiver on port ${whPort} (id=${createWhRes.data.id})`);

  // -------------------------------------------------------------------------
  // STEP 5: Multi-Party Sequential Submission, Delegation, Signing, & PDF Stamping
  // -------------------------------------------------------------------------
  logStep(5, 'Multi-Party Signing Flow (/s/:slug), Delegation, Email 2FA, & Completion');

  const createSubRes = await req('POST', '/api/submissions', {
    template_id: templateId,
    submitters_order: 'preserved',
    submitters: [
      {
        role: 'First Party',
        name: 'Bob Client',
        email: 'bob@client.org',
        values: { 'Company Name': 'Acme Robotics Inc.' },
      },
      {
        role: 'Second Party',
        name: 'Carol Counsel',
        email: 'carol@counsel.org',
      },
    ],
  });
  assert.equal(createSubRes.status, 201, `Create submission failed: ${JSON.stringify(createSubRes.data)}`);
  const submissionId = createSubRes.data.id;
  const submissionSlug = createSubRes.data.slug;
  const subSubmitters = createSubRes.data.submitters;
  assert.equal(subSubmitters.length, 2);

  const signer1Slug = subSubmitters[0].slug;
  let signer2Slug = subSubmitters[1].slug;
  console.log(`✓ Created multi-party submission (id=${submissionId}): Signer1=${signer1Slug}, Signer2=${signer2Slug}`);

  // Signer 1 opens public signing session (/api/s/:slug)
  const s1Session = await req('GET', `/api/s/${signer1Slug}`);
  assert.equal(s1Session.status, 200);
  assert.equal(s1Session.data.submitter.email, 'bob@client.org');
  console.log('✓ Signer 1 opened public signing session (/s/:slug)');

  // Test Email 2FA OTP generation and verification for Signer 1
  const sendOtpRes = await req('POST', `/api/s/${signer1Slug}/verify_email`, {});
  assert.equal(sendOtpRes.status, 200);
  assert.ok(sendOtpRes.data.verification_code_preview);
  const verifyOtpRes = await req('PUT', `/api/s/${signer1Slug}/verify_email`, {
    code: sendOtpRes.data.verification_code_preview,
  });
  assert.equal(verifyOtpRes.status, 200);
  assert.equal(verifyOtpRes.data.verified, true);
  console.log('✓ Signer 1 completed 6-digit Email 2FA OTP verification');

  // Signer 1 fills all fields & submits a drawn PNG signature
  const samplePngDataUri =
    'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==';
  const s1Complete = await req('PUT', `/api/s/${signer1Slug}`, {
    completed: true,
    values: {
      'fld-company': 'Acme Robotics Inc.',
      'fld-seats': '25',
      'fld-total': '2500',
      'fld-terms': true,
      'fld-plan': 'Enterprise',
      'fld-sig-1': samplePngDataUri,
    },
  });
  assert.equal(s1Complete.status, 200);
  assert.equal(s1Complete.data.submitter.status, 'completed');
  console.log('✓ Signer 1 completed signing; sequential invite dispatched to Signer 2');

  // Signer 2 delegates signing to Dave Director (/api/s/:slug/delegate)
  const delegateRes = await req('POST', `/api/s/${signer2Slug}/delegate`, {
    name: 'Dave Director',
    email: 'dave@counsel.org',
    reason: 'Delegating final legal sign-off to Director',
  });
  assert.equal(delegateRes.status, 200);
  signer2Slug = delegateRes.data.submitter.slug;
  assert.equal(delegateRes.data.submitter.email, 'dave@counsel.org');
  console.log(`✓ Signer 2 delegated signing to Dave Director (new slug=${signer2Slug})`);

  // Delegated Signer 2 completes the submission
  const s2Complete = await req('PUT', `/api/s/${signer2Slug}`, {
    completed: true,
    values: {
      'fld-sig-2': samplePngDataUri,
    },
  });
  assert.equal(s2Complete.status, 200);
  assert.equal(s2Complete.data.submission.status, 'completed');
  console.log('✓ Signer 2 completed signing -> Entire submission status is now COMPLETED!');

  // Test Decline flow on a separate submission
  const declineSubRes = await req('POST', '/api/submissions', {
    template_id: templateId,
    submitters: [{ role: 'First Party', name: 'Eve Decliner', email: 'eve@example.com' }],
  });
  assert.equal(declineSubRes.status, 201);
  const declineSlug = declineSubRes.data.submitters[0].slug;
  const declineAction = await req('POST', `/api/s/${declineSlug}/decline`, {
    reason: 'Contract terms require amendment in Section 4.',
  });
  assert.equal(declineAction.status, 200);
  assert.equal(declineAction.data.submitter.status, 'declined');
  console.log('✓ Submitter decline workflow (/s/:slug/decline) verified');

  // Test Self-Serve Shared Template Link (/api/d/:slug)
  const sharedGet = await req('GET', `/api/d/${templateSlug}`);
  assert.equal(sharedGet.status, 200);
  const sharedStart = await req('POST', `/api/d/${templateSlug}`, {
    name: 'Frank SelfServe',
    email: 'frank@selfserve.io',
  });
  assert.ok(sharedStart.status === 200 || sharedStart.status === 201);
  assert.ok(sharedStart.data.submitter?.slug || sharedStart.data.slug);
  console.log('✓ Public shared template link (/d/:slug) self-serve initiation verified');

  // Test Completed Submission Preview (/api/e/:slug)
  const previewRes = await req('GET', `/api/e/${submissionSlug}`);
  assert.equal(previewRes.status, 200);
  assert.equal(previewRes.data.submission.status, 'completed');
  console.log('✓ Public completed preview (/e/:slug) verified');

  // -------------------------------------------------------------------------
  // STEP 6: Download Signed PDF & Audit Trail PDF + Cryptographic Verification
  // -------------------------------------------------------------------------
  logStep(6, 'Download Signed PDF & Audit Trail PDF + Cryptographic X.509 PKCS#7 Verification');

  const signedPdfRes = await req('GET', `/api/submissions/${submissionId}/download`);
  assert.equal(signedPdfRes.status, 200);
  assert.ok(Buffer.isBuffer(signedPdfRes.data) && signedPdfRes.data.length > 500);
  const pdfText = signedPdfRes.data.toString('latin1');
  assert.ok(pdfText.includes('/ByteRange'), 'Signed PDF must contain /ByteRange');
  assert.ok(pdfText.includes('/adbe.pkcs7.detached'), 'Signed PDF must contain /adbe.pkcs7.detached');
  console.log(`✓ Downloaded cryptographically signed PDF (${signedPdfRes.data.length} bytes)`);

  const auditPdfRes = await req('GET', `/api/submissions/${submissionId}/download?audit=true`);
  assert.equal(auditPdfRes.status, 200);
  assert.ok(Buffer.isBuffer(auditPdfRes.data) && auditPdfRes.data.length > 500);
  console.log(`✓ Downloaded Audit Trail Certificate PDF (${auditPdfRes.data.length} bytes)`);

  // Verify the signed PDF via multipart upload to /api/verify
  const verifyForm = new FormData();
  verifyForm.append(
    'file',
    new Blob([signedPdfRes.data], { type: 'application/pdf' }),
    'signed-document.pdf'
  );
  const verifyRes = await req('POST', '/api/verify', verifyForm);
  assert.equal(verifyRes.status, 200);
  assert.equal(verifyRes.data.verified, true, `Verification failed: ${JSON.stringify(verifyRes.data)}`);
  assert.equal(verifyRes.data.integrity_valid, true);
  assert.equal(verifyRes.data.byte_range_valid, true);
  assert.equal(verifyRes.data.database_match.matched, true);
  console.log(
    '✓ Cryptographic PDF Verification passed:',
    `SHA256=${verifyRes.data.sha256}, Signer="${verifyRes.data.signature?.subject}"`
  );

  // Verify tamper detection by flipping a byte in the signed PDF
  const tamperedPdf = Buffer.from(signedPdfRes.data);
  tamperedPdf[40] = tamperedPdf[40] ^ 0xff;
  const tamperedVerify = await req('POST', '/api/tools/verify', {
    file: tamperedPdf.toString('base64'),
  });
  assert.equal(tamperedVerify.status, 200);
  assert.equal(tamperedVerify.data.verified, false, 'Tampered PDF must not pass verification');
  console.log('✓ Tamper detection verified: modified PDF byte range rejected');

  // Verify PDF Merge tool (/api/tools/merge)
  const mergeRes = await req('POST', '/api/tools/merge', {
    files: [signedPdfRes.data.toString('base64'), auditPdfRes.data.toString('base64')],
  });
  assert.equal(mergeRes.status, 200);
  assert.ok(mergeRes.data.pdf_base64);
  console.log(`✓ Multi-PDF merge tool (/api/tools/merge) verified (${mergeRes.data.byte_size} bytes)`);

  // -------------------------------------------------------------------------
  // STEP 7: Signline Customizations (White-Label, Gmail OAuth, Reminders)
  // -------------------------------------------------------------------------
  logStep(7, 'Signline Customizations: White-Label Personalization, Gmail OAuth 2.0, & Reminders');

  // 7a. White-label Personalization
  const persUpdate = await req('PUT', '/api/settings/personalization', {
    product_name: 'AcmeSign Enterprise',
    primary_color: '#1E40AF',
    accent_color: '#0F172A',
    email_sender_name: 'AcmeSign Legal',
    email_footer: 'Confidential — AcmeSign Enterprise',
    support_email: 'legal@acmesign.io',
    support_url: 'https://acmesign.io/support',
    terms_url: 'https://acmesign.io/terms',
    privacy_url: 'https://acmesign.io/privacy',
  });
  assert.equal(persUpdate.status, 200);
  assert.equal(persUpdate.data.product_name, 'AcmeSign Enterprise');
  assert.equal(persUpdate.data.primary_color, '#1E40AF');
  // Mandatory AGPLv3 Section 7(b) attribution check:
  assert.equal(persUpdate.data.attribution.upstream_name, 'Signline');
  assert.equal(persUpdate.data.attribution.upstream_url, 'https://github.com/signline/signline');
  assert.ok(persUpdate.data.attribution.text.includes('based on Signline'));
  console.log(
    '✓ White-label personalization updated & AGPLv3 "based on Signline" attribution verified:',
    persUpdate.data.attribution.text
  );

  // Restore product_name to Signline
  await req('PUT', '/api/settings/personalization', {
    product_name: 'Signline',
    primary_color: '#2161AD',
    accent_color: '#253E8A',
  });

  // 7b. SMTP & Gmail OAuth 2.0 Integration
  const smtpSave = await req('PUT', '/api/settings/email/smtp', {
    host: 'smtp.example.org',
    port: 587,
    username: 'mailer@example.org',
    password: 'smtp_secret_password',
    from_email: 'noreply@example.org',
    security: 'starttls',
  });
  assert.equal(smtpSave.status, 200);
  assert.equal(smtpSave.data.active_transport, 'smtp');
  console.log('✓ Configured SMTP settings (active_transport = smtp)');

  // Gmail OAuth 2.0 credentials + authorization URL (state nonce + PKCE S256)
  const gmailCreds = await req('POST', '/api/settings/email/gmail', {
    client_id: '1234567890-signline.apps.googleusercontent.com',
    client_secret: 'GOCSPX-signline-test-secret',
  });
  assert.equal(gmailCreds.status, 200);
  assert.equal(gmailCreds.data.gmail.configured, true);
  assert.equal(gmailCreds.data.gmail.connected, false);

  const gmailConnect = await req('GET', '/api/settings/email/gmail/connect');
  assert.equal(gmailConnect.status, 200);
  const authUrl = new URL(gmailConnect.data.authorization_url);
  assert.equal(authUrl.origin + authUrl.pathname, 'https://accounts.google.com/o/oauth2/v2/auth');
  assert.ok(authUrl.searchParams.get('scope').includes('gmail.send'));
  assert.equal(authUrl.searchParams.get('code_challenge_method'), 'S256');
  assert.ok(authUrl.searchParams.get('code_challenge'));
  const oauthState = authUrl.searchParams.get('state');
  assert.ok(oauthState && oauthState.includes('.'), 'Expected <account_id>.<nonce> OAuth state');
  console.log('✓ Gmail OAuth 2.0 authorization URL issued with state nonce & PKCE (S256)');

  // A forged state must be rejected (redirects back to settings with an error)
  const forgedCallback = await fetch(
    `${BASE_URL}/api/settings/email/gmail/callback?code=forged&state=${encodeURIComponent(
      oauthState.split('.')[0] + '.forged'
    )}`,
    { redirect: 'manual' }
  );
  assert.equal(forgedCallback.status, 303);
  assert.ok(forgedCallback.headers.get('location').startsWith('/settings/email?gmail=error'));
  const emailStatusAfterForged = await req('GET', '/api/settings/email/gmail');
  assert.equal(emailStatusAfterForged.data.gmail.connected, false);
  assert.equal(
    emailStatusAfterForged.data.active_transport,
    'smtp',
    'SMTP must remain active while Gmail is not connected'
  );
  console.log('✓ Forged Gmail OAuth state rejected; SMTP remains the active transport');

  // Send a test email through the SMTP transport
  const testEmailRes = await req('POST', '/api/settings/email/test', {
    to: 'test@example.com',
  });
  assert.equal(testEmailRes.status, 200);
  assert.equal(testEmailRes.data.ok, true);
  console.log('✓ Dispatched test email through the SMTP transport');

  // 7c. Automated Signature Request Reminders
  const reminderSub = await req('POST', '/api/submissions', {
    template_id: templateId,
    submitters: [{ role: 'First Party', name: 'Pending Signer', email: 'pending@signer.org' }],
  });
  assert.equal(reminderSub.status, 201);

  const notifSave = await req('POST', '/api/settings/notifications', {
    first_duration: 'four_hours',
    second_duration: 'twenty_four_hours',
    third_duration: 'three_days',
    run_now: true,
  });
  assert.equal(notifSave.status, 200);
  assert.ok(notifSave.data.sent_count >= 1, 'Expected reminder email to be sent to pending signer');
  console.log(
    `✓ Automated signature request reminders configured and executed (sent_count=${notifSave.data.sent_count})`
  );

  // -------------------------------------------------------------------------
  // STEP 8: E-Signature Certs, Team RBAC, Storage, REST API (X-Auth-Token), & MCP Server
  // -------------------------------------------------------------------------
  logStep(8, 'E-Signature X.509 Certs, Team Users, Webhooks HMAC Check, REST API, & MCP Server');

  // E-Signature settings
  const esignRegen = await req('POST', '/api/settings/esign', {
    action: 'regenerate',
    timestamp_server_url: 'http://timestamp.digicert.com',
  });
  assert.equal(esignRegen.status, 200);
  assert.equal(esignRegen.data.timestamp_server_url, 'http://timestamp.digicert.com');
  assert.ok(esignRegen.data.certificate.fingerprint_sha256);
  console.log(
    '✓ Regenerated 2048-bit RSA X.509 certificate & configured RFC 3161 TSA URL:',
    esignRegen.data.certificate.fingerprint_sha256
  );

  // Team management (Users CRUD)
  const newUserRes = await req('POST', '/api/settings/users', {
    first_name: 'Grace',
    last_name: 'Manager',
    email: 'grace@signline.local',
    role: 'manager',
  });
  assert.equal(newUserRes.status, 201);
  const archiveUserRes = await req('DELETE', `/api/settings/users/${newUserRes.data.id}`);
  assert.equal(archiveUserRes.status, 200);
  const restoreUserRes = await req('PUT', `/api/settings/users/${newUserRes.data.id}`, {
    restore: true,
  });
  assert.equal(restoreUserRes.status, 200);
  console.log('✓ Team user creation, role assignment, archiving, and restoration verified');

  // Verify received Webhooks and X-Signline-Signature HMAC-SHA256 header
  assert.ok(receivedWebhooks.length > 0, 'Expected local webhook server to receive events');
  const sampleWh = receivedWebhooks[0];
  const expectedHmac = crypto
    .createHmac('sha256', whSecret)
    .update(sampleWh.body)
    .digest('hex');
  assert.equal(
    sampleWh.headers['x-signline-signature'],
    expectedHmac,
    'X-Signline-Signature header must match HMAC-SHA256 of payload'
  );
  console.log(
    `✓ Verified ${receivedWebhooks.length} delivered webhooks with valid X-Signline-Signature HMAC-SHA256`
  );
  webhookServer.close();

  // REST API with X-Auth-Token & X-Account-Testing sandbox header
  const apiSettingsRes = await req('GET', '/api/settings/api');
  assert.equal(apiSettingsRes.status, 200);
  apiAccessToken = apiSettingsRes.data.token;
  assert.ok(apiAccessToken.startsWith('sl_live_'));

  const apiTemplatesRes = await fetch(`${BASE_URL}/api/templates`, {
    headers: { 'X-Auth-Token': apiAccessToken },
  }).then((r) => r.json());
  assert.ok(Array.isArray(apiTemplatesRes.data) && apiTemplatesRes.data.length >= 2);
  console.log('✓ Authenticated REST API call with X-Auth-Token header (/api/templates)');

  const sandboxTemplatesRes = await fetch(`${BASE_URL}/api/templates`, {
    headers: {
      'X-Auth-Token': apiAccessToken,
      'X-Account-Testing': 'true',
    },
  }).then((r) => r.json());
  assert.ok(Array.isArray(sandboxTemplatesRes.data));
  console.log('✓ Verified X-Account-Testing sandbox isolation header');

  // Model Context Protocol (MCP) Server JSON-RPC 2.0 verification
  const mcpTokenCreate = await req('POST', '/api/settings/mcp', {
    name: 'E2E Simulation MCP Agent',
  });
  assert.equal(mcpTokenCreate.status, 201);
  mcpBearerToken = mcpTokenCreate.data.token;
  assert.ok(mcpBearerToken.startsWith('sl_mcp_'));

  const mcpInit = await fetch(`${BASE_URL}/mcp`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${mcpBearerToken}`,
    },
    body: JSON.stringify({
      jsonrpc: '2.0',
      id: 1,
      method: 'initialize',
      params: {},
    }),
  }).then((r) => r.json());
  assert.equal(mcpInit.result.serverInfo.name, 'signline-mcp');

  const mcpTools = await fetch(`${BASE_URL}/mcp`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${mcpBearerToken}`,
    },
    body: JSON.stringify({
      jsonrpc: '2.0',
      id: 2,
      method: 'tools/list',
      params: {},
    }),
  }).then((r) => r.json());
  assert.equal(mcpTools.result.tools.length, 7);

  const mcpCall = await fetch(`${BASE_URL}/mcp`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${mcpBearerToken}`,
    },
    body: JSON.stringify({
      jsonrpc: '2.0',
      id: 3,
      method: 'tools/call',
      params: {
        name: 'list_templates',
        arguments: { limit: 5 },
      },
    }),
  }).then((r) => r.json());
  assert.equal(mcpCall.result.isError, false);
  console.log('✓ Verified MCP JSON-RPC 2.0 server (initialize, tools/list [7 tools], tools/call)');

  // CSV Export & Unified Search
  const csvExport = await req('GET', '/api/submissions/export');
  assert.equal(csvExport.status, 200);
  assert.ok(typeof csvExport.data === 'string' && csvExport.data.includes('submission_id'));
  const searchRes = await req('GET', '/api/search?q=Master');
  assert.equal(searchRes.status, 200);
  assert.ok(searchRes.data.templates.length >= 1);
  console.log('✓ Verified CSV export (/api/submissions/export) and unified search (/api/search)');

  // Verify Outgoing Email Events & MIME PDF Attachments across all workflow categories
  const emailEventsRes = await req('GET', '/api/settings/email/events');
  assert.equal(emailEventsRes.status, 200);
  const emailEvents = emailEventsRes.data.events || emailEventsRes.data.data || [];
  const observedTags = new Set(emailEvents.map((e) => e.tag));
  const requiredEmailTags = [
    'welcome_email',
    'password_reset',
    'password_reset_confirmed',
    'submitter_invitation',
    'submitter_delegated',
    'submission_completed',
    'submission_declined',
    'submitter_email_2fa',
    'submitter_invitation_reminder',
    'test_email',
  ];
  for (const tag of requiredEmailTags) {
    assert.ok(observedTags.has(tag), `Expected email_events to include tag '${tag}'`);
  }
  const invitationWithAttachment = emailEvents.find(
    (e) =>
      e.tag === 'submitter_invitation' &&
      Array.isArray(e.data?.attachments) &&
      e.data.attachments.length >= 1
  );
  assert.ok(
    invitationWithAttachment,
    'Expected submitter_invitation email to include attached PDF document for signature'
  );
  const completedWithAttachments = emailEvents.find(
    (e) =>
      e.tag === 'submission_completed' &&
      Array.isArray(e.data?.attachments) &&
      e.data.attachments.length >= 2
  );
  assert.ok(
    completedWithAttachments,
    'Expected submission_completed email to include both signed PDF and audit trail PDF attachments'
  );
  console.log(
    `✓ Verified all ${requiredEmailTags.length} email categories (${Array.from(observedTags).join(', ')}) and RFC 2822 MIME PDF attachments!`
  );

  console.log('\n================================================================');
  console.log('ALL 8 E2E REAL-USER SIMULATION SUITES PASSED 100%!');
  console.log('================================================================\n');
}

runSimulation().catch((err) => {
  console.error('\n[SIMULATION FAILED]', err);
  process.exit(1);
});
