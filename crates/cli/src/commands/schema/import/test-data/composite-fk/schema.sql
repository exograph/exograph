-- events.tenant_id is a shared column between sources and events, but should be written as a scalar field since there is not "tenant" model
-- events.(tenant_id, source_id) should be writen as a relation

CREATE TABLE IF NOT EXISTS sources (
  tenant_id text NOT NULL,
  source_id TEXT NOT NULL,
  name TEXT,

  CONSTRAINT sources_pkey PRIMARY KEY (tenant_id, source_id)
);

CREATE TABLE IF NOT EXISTS events (
    tenant_id text NOT NULL,
    event_id text NOT NULL,
    source_id text NOT NULL,
    name text NOT NULL,
    CONSTRAINT events_pkey PRIMARY KEY (tenant_id, event_id),
    CONSTRAINT fk_events_sources FOREIGN KEY (tenant_id, source_id) REFERENCES sources (tenant_id, source_id)
);
