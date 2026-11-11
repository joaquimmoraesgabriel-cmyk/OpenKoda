// Positive synthetic fixtures explicitly review only their named methods. This
// helper calls the public catalog/create contract; it never edits policy rows,
// approves an execution, falls back to legacy create, or selects all methods.
import assert from 'node:assert/strict';

export const unboundedScopeWindow = () => ({ startsAt: null, endsAt: null });

export async function reviewedAssessment(request, payload, methods, window = unboundedScopeWindow()) {
  assert.equal(Object.hasOwn(payload, 'scope'), false, 'The fixture scope must be supplied exactly once.');
  assert.ok(Array.isArray(methods), 'Each fixture must explicitly choose its required methods, including an empty list.');
  assert.equal(new Set(methods).size, methods.length, 'Fixture methods must be unique.');
  const options = await request('assessment.scopeOptions', { projectId: payload.projectId });
  assert.equal(options.schema, 'openkoda.scope-options.v1');
  assert.equal(options.projectId, payload.projectId);
  assert.equal(options.kind, payload.kind);
  assert.equal(options.target, payload.target);
  assert.equal(options.canCreate, true, 'The native catalog must permit this synthetic project.');
  for (const method of methods) {
    assert.equal(options.methodCatalog.find(item => item.id === method)?.allowed, true, `Native catalog does not allow ${method}.`);
  }
  const assessment = await request('assessment.create', { ...payload, scope: {
    methods: [...methods], window, projectDigest: options.projectDigest, catalogVersion: options.catalogVersion, acknowledgeScope: true,
  } });
  const status = await request('assessment.scopeStatus', { assessmentId: assessment.id });
  assert.equal(status.phase, 'draft');
  assert.deepEqual(status.revision.body.methods, [...methods].sort());
  assert.equal(status.revision.review?.acknowledgeScope, true);
  assert.equal(status.frozen, null);
  return assessment;
}
