-- project_provisioning_attempts: fences provisioning attempts and deletion.
--
--   DELETING                 deletion started; it drops the project's
--                            infrastructure and can be retried
--   provisioning_attempt     the attempt allowed to record how provisioning
--                            ended; a newer attempt replaces it
--   provisioning_started_at  when that attempt started; a CREATING project
--                            whose attempt is older than the stale window
--                            lost it (e.g. the server restarted mid-way)

ALTER TYPE public.project_status ADD VALUE 'DELETING';

ALTER TABLE public.projects
    ADD COLUMN provisioning_attempt uuid,
    ADD COLUMN provisioning_started_at timestamp with time zone;

-- Attempts started before this migration have no start time; their last
-- update is the closest one, so they can still go stale.
UPDATE public.projects
SET provisioning_started_at = updated_at
WHERE status = 'CREATING';
