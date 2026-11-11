// Public IPC helpers for exclusively owned synthetic native test profiles.
// Explicitly grants report/recovery policy and classifies fixture subjects. Never
// use against a personal profile. No SQL, key, target operation, provider or WSL.
import assert from 'node:assert/strict';
import { randomUUID, createHash } from 'node:crypto';

const categories = ['human_text', 'target_metadata', 'execution_summary', 'evidence_excerpt', 'source_excerpt', 'finding_text', 'activity_metadata'];
const sensitivities = ['public', 'internal', 'sensitive'];
const sha256 = text => createHash('sha256').update(text).digest('hex');

async function projectPolicy(request, projectId) {
  const status = await request('data.policy.status', { projectId });
  if (status.currentRevision) {
    assert.equal(status.active, true, 'The fixture helper must not reactivate revoked/restored policy implicitly.');
    return { status, policyRef: { projectId, revisionId: status.currentRevision.id, digest: status.currentRevision.digest } };
  }
  const body = structuredClone(status.defaultBody);
  const report = body.destinations.find(rule => rule.destination === 'report_export');
  assert.ok(report, 'The native default policy must define report_export.');
  Object.assign(report, { enabled: true, categories: [...categories], sensitivities: [...sensitivities],
    targetCatalogIds: status.sourceOptions.filter(source => !source.inactive).map(source => source.targetCatalogId), allowOriginalEvidence: false });
  body.recoveryBackup = { enabled: true, sensitivities: [...sensitivities] };
  // Remote AI and local Kali remain disabled; callers review those independently.
  const preview = await request('data.policy.prepare', { projectId, expectedStateDigest: status.stateDigest, body });
  assert.equal(preview.canCommit, true, 'The fixture data policy must be reviewable.');
  const saved = await request('data.policy.commit', { review: preview.review, idempotencyKey: randomUUID(), acknowledgePolicy: true });
  assert.equal(saved.saved, true);
  return { status: saved.status, policyRef: saved.policyRef };
}

/** Explicitly authorize report/recovery data in a synthetic reviewed assessment.
 * Existing policy is preserved; revoked policy is never reactivated. Returns
 * {status, subjects, policyRef, contextTargetRefs}. This does not grant scope.
 */
export async function authorizeData(request, assessmentId) {
  const snapshot = await request('snapshot');
  const assessment = snapshot.assessments.find(item => item.id === assessmentId);
  assert.ok(assessment, 'Synthetic assessment must exist.');
  const projectId = assessment.projectId;
  const { policyRef } = await projectPolicy(request, projectId);
  let status = await request('data.policy.status', { projectId, assessmentId });
  if (!status.binding) {
    const preview = await request('data.binding.prepare', { assessmentId, policyRef, expectedStateDigest: status.stateDigest });
    assert.equal(preview.canCommit, true, 'A separate prospective data binding is required.');
    const adopted = await request('data.binding.commit', { review: preview.review, idempotencyKey: randomUUID(), acknowledgeProspectiveAdoption: true });
    assert.equal(adopted.adopted, true); assert.equal(adopted.binding.origin, 'explicit_adoption');
  }
  const listing = await request('data.classification.subjects', { assessmentId });
  const origins = listing.subjects.filter(row => row.subject.kind === 'scope_source').flatMap(row => row.originRefs);
  assert.ok(origins.length > 0, 'Review/adopt the operational scope before authorizing data; no scope is fabricated by this helper.');
  const priority = row => ({ scope_source: 0, assessment_metadata: 1, evidence: 2 }[row.subject.kind] ?? (row.subject.producer === 'finding' ? 3 : row.subject.producer === 'validation' ? 4 : 5));
  for (const listed of [...listing.subjects].sort((a, b) => priority(a) - priority(b))) {
    // Earlier evidence classification/association can resolve a derived origin.
    // Re-read the host projection before deciding whether an association is still
    // needed; never replace an origin that became known during this loop.
    const currentListing = await request('data.classification.subjects', { assessmentId });
    const row = currentListing.subjects.find(item => JSON.stringify(item.subject) === JSON.stringify(listed.subject));
    assert.ok(row, 'The subject disappeared during synthetic classification.');
    // A valid own record may have an effective unclassified state because a
    // contributor is unresolved. Do not overwrite its own sensitivity decision.
    if (row.classification.classificationRef) continue;
    const payload = { subject: row.subject, expectedSourceFingerprint: row.sourceFingerprint, sensitivity: 'public',
      reason: 'Owned synthetic test subject explicitly reviewed for this native fixture.' };
    if (['evidence', 'derived'].includes(row.subject.kind) && !row.originVerified && row.originRefs.length === 0) {
      payload.associationRefs = currentListing.subjects.filter(item => item.subject.kind === 'scope_source').flatMap(item => item.originRefs);
      assert.ok(payload.associationRefs.length > 0, 'A human data association requires current reviewed sources.');
    }
    const preview = await request('data.classification.prepare', payload);
    assert.equal(preview.proposal.secretMaterial, false, 'A fixture containing recognized secret material must be fixed, not bypassed.');
    assert.equal(preview.downgrade, false, 'This helper does not silently downgrade a prior sensitivity decision.');
    const classified = await request('data.classification.commit', { review: preview.review, idempotencyKey: randomUUID(), acknowledgeClassification: true });
    assert.equal(classified.classified, true);
  }
  const current = await request('data.classification.subjects', { assessmentId });
  assert.ok(current.subjects.every(row => row.classification.sensitivity !== 'unclassified'), 'Every contributor must have a current explicit classification.');
  status = await request('data.policy.status', { projectId, assessmentId });
  const contextTargetRefs = [...new Map(origins.filter(origin => origin.targetRef).map(origin => [JSON.stringify(origin.targetRef), origin.targetRef])).values()];
  return { status, subjects: current.subjects, policyRef, contextTargetRefs };
}

/** Returns the report.commit result. The selection is reconstructed from native
 * subject DTOs and explicitly confirmed. includeEvidence only controls files.
 */
export async function reviewedReport(request, {
  assessmentId, includeEvidence = true, evidenceIds, derivedSubjects, categories: chosenCategories,
  contextTargetRefs, packageSensitivity = 'public',
}) {
  assert.ok(assessmentId, 'assessmentId is required for a reviewed synthetic report.');
  const authorized = await authorizeData(request, assessmentId);
  const payload = {
    assessmentId, format: 'markdown_bundle',
    evidenceIds: evidenceIds ?? (includeEvidence ? authorized.subjects.filter(row => row.subject.kind === 'evidence').map(row => row.subject.evidenceId) : []),
    derivedSubjects: derivedSubjects ?? authorized.subjects.filter(row => row.subject.kind === 'derived').map(row => row.subject),
    categories: chosenCategories ?? [...categories], contextTargetRefs: contextTargetRefs ?? authorized.contextTargetRefs, packageSensitivity,
  };
  const preview = await request('report.prepare', payload);
  assert.equal(preview.canExport, true, `Synthetic report blocked: ${JSON.stringify(preview.reasonCodes)}`);
  assert.ok(preview.dataReview, 'A report cannot be committed without a native review.');
  const report = await request('report.commit', { dataReview: preview.dataReview, acknowledgeExport: true, acknowledgeSensitive: preview.requiresSensitiveConfirmation === true });
  assert.equal(report.completed, true); return report;
}

/** Returns backup.commit. Recovery includes the whole synthetic profile; callers
 * cannot silently select only some projects while copying all database rows.
 */
export async function reviewedBackup(request, { projectIds, packageSensitivity = 'public' } = {}) {
  const snapshot = await request('snapshot');
  const allProjects = snapshot.projects.map(project => project.id).sort();
  const selected = [...(projectIds ?? allProjects)].sort();
  assert.deepEqual(selected, allProjects, 'Recovery backup must explicitly include every project in the synthetic profile.');
  for (const projectId of selected) await projectPolicy(request, projectId);
  for (const assessment of snapshot.assessments) await authorizeData(request, assessment.id);
  const preview = await request('backup.prepare', { projectIds: selected, packageSensitivity, acknowledgeRecoveryPurpose: true });
  assert.equal(preview.canBackup, true, `Synthetic recovery backup blocked: ${JSON.stringify(preview.reasonCodes)}`);
  assert.ok(preview.dataReview, 'A recovery backup requires its native data review.');
  const backup = await request('backup.commit', { dataReview: preview.dataReview, acknowledgeRecoveryBackup: true, acknowledgeSensitive: preview.requiresSensitiveConfirmation === true });
  assert.equal(backup.verified, true); return backup;
}

/** Returns the explicitly reviewed representation as a string, never a DTO.
 * May contain redactions; the original fingerprint is not rewritten. This does
 * not create an output policy or grant permission to AI/Kali/report/backup.
 */
export async function reviewedEvidenceText(request, assessmentId, evidenceId, expectedRawSha256) {
  assert.match(expectedRawSha256, /^[a-f0-9]{64}$/, 'Pass rawSha256 from the evidence DTO or the owned forensic fixture.');
  await request('data.session.select', { assessmentId });
  let offset = 0, projectionSha256, totalBytes, result = '';
  for (let index = 0; index < 128; index++) {
    const preview = await request('evidence.access.prepare', { assessmentId, evidenceId, expectedRawSha256, offset, maxBytes: 32768,
      ...(projectionSha256 ? { expectedProjectionSha256: projectionSha256 } : {}) });
    assert.equal(preview.canRead, true); assert.equal(Object.hasOwn(preview, 'text'), false, 'Prepare must not expose raw text.');
    const value = await request('evidence.access.read', { review: preview.review, acknowledgeLocalRead: true, acknowledgeSensitive: preview.classification.sensitivity === 'sensitive' });
    assert.equal(value.rawSha256, expectedRawSha256); assert.equal(value.rangeStart, offset);
    assert.equal(Buffer.byteLength(value.text), value.returnedBytes); assert.equal(value.rangeEnd - value.rangeStart, value.returnedBytes);
    if (projectionSha256) assert.equal(value.projectionSha256, projectionSha256);
    if (totalBytes !== undefined) assert.equal(value.totalBytes, totalBytes);
    projectionSha256 = value.projectionSha256; totalBytes = value.totalBytes;
    result += value.text; offset = value.rangeEnd;
    if (!value.hasMore) {
      assert.equal(offset, totalBytes); assert.equal(Buffer.byteLength(result), totalBytes); assert.equal(sha256(result), projectionSha256);
      return result;
    }
    assert.ok(value.returnedBytes > 0, 'A continuing UTF-8 window must advance.');
  }
  throw new Error('Synthetic evidence exceeds the bounded reviewed window loop.');
}
