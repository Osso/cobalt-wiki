"""Guarded operations for the inspected Cobalt navigation backlog.

Stop producers/consumers before use. Journal SELECT results durably before
APPLY; RESTORE accepts that journal. Only due, unreceived depth-2 navigation
jobs for site 6000000 qualify, with an identical pending keeper still present.
"""

_MATCH = r"""
local function matches(c)
  local r=c.record
  if r.id==c.keeper_id or r.rc~=cjson.null or r.fr~=cjson.null then return false end
  if redis.call('HGET',KEYS[2],r.id)~=r.payload then return false end
  if redis.call('ZSCORE',KEYS[1],r.id)~=r.score then return false end
  if not tonumber(r.score) or tonumber(r.score)>tonumber(ARGV[2]) then return false end
  if redis.call('HGET',KEYS[2],r.id..':rc') or redis.call('HGET',KEYS[2],r.id..':fr') then return false end
  if redis.call('HGET',KEYS[2],c.keeper_id)~=r.payload then return false end
  local score=redis.call('ZSCORE',KEYS[1],c.keeper_id)
  if not score or tonumber(score)>tonumber(ARGV[2]) then return false end
  if redis.call('HGET',KEYS[2],c.keeper_id..':rc') or redis.call('HGET',KEYS[2],c.keeper_id..':fr') then return false end
  local ok,job=pcall(cjson.decode,r.payload)
  if not ok or type(job)~='table' or job.job~='rerender_page' then return false end
  local data=job.data
  return type(data)=='table' and data.type=='nav' and data.depth==2
    and type(data.id)=='table' and data.id.site_id==6000000
end
"""

SELECT = (
    _MATCH
    + r"""
local selected={}
for _,candidate in ipairs(cjson.decode(ARGV[1])) do
  if matches(candidate) then table.insert(selected,candidate) end
end
return cjson.encode(selected)
"""
)

APPLY = (
    _MATCH
    + r"""
local candidates=cjson.decode(ARGV[1])
for _,candidate in ipairs(candidates) do
  if not matches(candidate) then return redis.error_reply('queue changed before guarded deletion') end
end
local removed=0
for _,candidate in ipairs(candidates) do
  local id=candidate.record.id
  removed=removed+redis.call('ZREM',KEYS[1],id)
  redis.call('HDEL',KEYS[2],id,id..':rc',id..':fr')
end
return removed
"""
)

RESTORE = r"""
local candidates=cjson.decode(ARGV[1])
for _,candidate in ipairs(candidates) do
  local r=candidate.record
  local payload=redis.call('HGET',KEYS[2],r.id)
  local score=redis.call('ZSCORE',KEYS[1],r.id)
  if (payload and payload~=r.payload) or (score and score~=r.score)
    or redis.call('HGET',KEYS[2],r.id..':rc') or redis.call('HGET',KEYS[2],r.id..':fr') then
    return redis.error_reply('queue changed before guarded restoration')
  end
end
local restored=0
for _,candidate in ipairs(candidates) do
  local r=candidate.record
  redis.call('HSET',KEYS[2],r.id,r.payload)
  restored=restored+redis.call('ZADD',KEYS[1],r.score,r.id)
end
return restored
"""
