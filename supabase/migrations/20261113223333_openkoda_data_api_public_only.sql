-- Hosted koda-dev was configured with no exposed schemas. Only the service-role
-- RPCs live in public. Keep openkoda_private outside the PostgREST cache.
alter role authenticator set pgrst.db_schemas = 'public';
notify pgrst, 'reload config';
notify pgrst, 'reload schema';
