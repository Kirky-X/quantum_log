# InfluxDB API Token Configuration

**SECURITY WARNING**: Do not store API tokens or credentials in this file!

## Recommended Setup

1. Set environment variables instead:
   ```bash
   export INFLUXDB_TOKEN="your_actual_token_here"
   export INFLUXDB_ORG="kirky"
   export INFLUXDB_BUCKET="test"
   ```

2. Or use a `.env` file (make sure it's in .gitignore):
   ```
   INFLUXDB_TOKEN=your_actual_token_here
   INFLUXDB_ORG=kirky
   INFLUXDB_BUCKET=test
   ```

3. For testing purposes, you can temporarily place your token here,
   but remember to remove it before committing to version control.

## Token Format
Your InfluxDB token should look like:
`HNAmaorKV3HWVJaHCEzxZFNK4qjS3EecfcifbiTnh6KpJ335BWp_Eu0Okut1-AekVP9nrDBSVX_Fmj7EdbagSQ==`