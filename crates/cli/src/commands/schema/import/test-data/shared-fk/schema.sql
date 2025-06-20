-- memberships.membership_tenant_id is parts of two foreign keys:
-- memberships -> members (member_tenant_id, member_id)
-- memberships -> tenants (tenant_id)

CREATE TABLE members (
	member_id TEXT,
	member_tenant_id TEXT,
	member_name TEXT,
	PRIMARY KEY (member_tenant_id, member_id)
);

CREATE TABLE memberships (
	membership_id TEXT PRIMARY KEY,
	membership_tenant_id TEXT,
	membership_member_id TEXT,
	membership_name TEXT
);

CREATE TABLE tenants (
	tenant_id TEXT PRIMARY KEY,
	tenant_name TEXT
);

ALTER TABLE memberships ADD CONSTRAINT fk_memberships_members FOREIGN KEY (membership_tenant_id, membership_member_id) REFERENCES members(member_tenant_id, member_id);

ALTER TABLE memberships ADD CONSTRAINT fk_memberships_tenants FOREIGN KEY (membership_tenant_id) REFERENCES tenants(tenant_id);
