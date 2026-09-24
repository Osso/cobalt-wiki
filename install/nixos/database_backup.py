"""Stream a pg_dump of the Cobalt database to S3 (Cloudflare R2), then check
the stored object's size. The bucket's lifecycle rule expires old dumps.
Invoked by cobalt-wiki-database-backup.service.

    database_backup.py PG_DUMP MC ENDPOINT BUCKET

The environment carries the PG* connection settings and
BACKUP_S3_ACCESS_KEY_ID / BACKUP_S3_SECRET_ACCESS_KEY (keys limited to BUCKET).
"""

import datetime
import json
import os
import subprocess
import sys
import urllib.parse

pg_dump, mc, endpoint, bucket = sys.argv[1:5]
runtime = os.environ["RUNTIME_DIRECTORY"]

url = urllib.parse.urlsplit(endpoint)
access = urllib.parse.quote(os.environ["BACKUP_S3_ACCESS_KEY_ID"], safe="")
secret = urllib.parse.quote(os.environ["BACKUP_S3_SECRET_ACCESS_KEY"], safe="")
env = {**os.environ, "MC_HOST_backup": f"{url.scheme}://{access}:{secret}@{url.netloc}"}
mc_args = [mc, "--config-dir", runtime, "--quiet", "--no-color"]

name = datetime.datetime.now(datetime.UTC).strftime("cobalt_wiki-%Y%m%dT%H%M%SZ.dump")
target = f"backup/{bucket}/{name}"

dump = subprocess.Popen([pg_dump, "--format=custom", "--compress=9", "cobalt_wiki"], stdout=subprocess.PIPE)
upload = subprocess.run([*mc_args, "pipe", target], stdin=dump.stdout, env=env)
dump.stdout.close()
if dump.wait() != 0 or upload.returncode != 0:
    subprocess.run([*mc_args, "rm", target], env=env)
    sys.exit(f"backup failed: pg_dump exit {dump.returncode}, upload exit {upload.returncode}")

stat = subprocess.run([*mc_args, "stat", "--json", target], env=env, capture_output=True, text=True, check=True)
size = json.loads(stat.stdout)["size"]
if size < 1024:
    sys.exit(f"backup {name} is only {size} bytes")
print(f"backed up {name}: {size} bytes")
