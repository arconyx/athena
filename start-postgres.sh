#!/usr/bin/env bash
echo "Starting temporary postgres instance in $PGDATA"

# Create necessary directories
mkdir -p "$PGDATA" "$PGHOST"

# Initialize DB if not already done
if [ ! -f "$PGDATA/PG_VERSION" ]; then
  pg_ctl initdb -o "-U postgres"
fi

# Write PostgreSQL configuration
cat > "$PGDATA/postgresql.conf" <<EOF
# Add Custom Settings
log_directory = 'pg_log'
log_filename = 'postgresql-%Y-%m-%d_%H%M%S.log'
logging_collector = on
# Unix socket settings
unix_socket_directories = '$PGHOST'
EOF

# Start PostgreSQL
pg_ctl -o "-k $PGHOST" start

# Create the 'athena' database if it doesn't exist
psql -U postgres -c "SELECT 1 FROM pg_database WHERE datname = 'athena'" | grep -q 1 || psql -U postgres -c "CREATE DATABASE athena"

# Aliases for convenience (only works if sourced)
alias fin="pg_ctl stop && exit"
alias pg="psql -h $PGHOST -U postgres"

echo "Database running. Stop it with 'pg_ctl stop'"
